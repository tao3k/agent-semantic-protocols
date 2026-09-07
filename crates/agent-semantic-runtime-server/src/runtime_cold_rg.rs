// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! One bounded ripgrep process over an immutable content-generation corpus.

use std::collections::BTreeSet;
use std::time::Duration;

const MAX_NATIVE_RG_MATCHES: u32 = 4096;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct RuntimeResidentLexicalReceipt {
    pub content_generation_digest: String,
    pub coverage_input_digest: String,
    pub candidate_owner_paths: Vec<String>,
    pub elapsed_micros: u64,
    pub process_count: u8,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct RuntimeNativeRgAxisReceipt {
    pub content_generation_digest: String,
    pub candidate_owner_paths: Vec<String>,
    pub branch_candidate_owner_paths: Vec<Vec<String>>,
    pub branch_matches: Vec<Vec<RuntimeRgMatch>>,
    pub process_count: u8,
    pub truncated: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct RuntimeRgMatch {
    pub owner_path: String,
    pub owner_line: u64,
}

pub(crate) async fn execute_runtime_native_rg_blocks(
    corpus: &agent_semantic_search::ColdRgCorpusArtifact,
    blocks: &[Vec<String>],
    limit: u32,
    deadline: Duration,
) -> Result<RuntimeNativeRgAxisReceipt, String> {
    if blocks.is_empty() || !(1..=MAX_NATIVE_RG_MATCHES).contains(&limit) || deadline.is_zero() {
        return Err("native rg axis is outside the bounded Runtime envelope".to_owned());
    }
    let receipt = tokio::time::timeout(
        deadline,
        agent_semantic_search::execute_content_bound_native_rg_blocks(
            corpus,
            blocks,
            limit as usize,
        ),
    )
    .await
    .map_err(|_| "native rg axis deadline exceeded; process capability dropped".to_owned())??;
    Ok(RuntimeNativeRgAxisReceipt {
        content_generation_digest: corpus.receipt.content_generation_digest.clone(),
        candidate_owner_paths: receipt.axis.candidate_owner_paths,
        branch_candidate_owner_paths: receipt.axis.branch_candidate_owner_paths,
        branch_matches: receipt
            .branch_matches
            .into_iter()
            .map(|branch| {
                branch
                    .into_iter()
                    .map(|item| RuntimeRgMatch {
                        owner_path: item.owner_path,
                        owner_line: item.owner_line,
                    })
                    .collect()
            })
            .collect(),
        process_count: u8::try_from(receipt.process_count).unwrap_or(u8::MAX),
        truncated: receipt.truncated,
    })
}

pub(crate) async fn execute_runtime_resident_lexical_prefilter(
    corpus: &agent_semantic_search::ColdRgCorpusArtifact,
    query: &str,
    limit: u32,
    deadline: Duration,
) -> Result<RuntimeResidentLexicalReceipt, String> {
    if query.trim().is_empty() || !(1..=100).contains(&limit) || deadline < Duration::from_micros(1)
    {
        return Err("resident lexical request is outside the bounded Runtime envelope".to_owned());
    }
    let terms = agent_semantic_search::source_index_lookup_terms(query)
        .into_iter()
        .filter(|term| !term.trim().is_empty())
        .collect::<BTreeSet<_>>();
    if terms.is_empty() {
        return Err("resident lexical request has no normalized terms".to_owned());
    }
    let coverage_input_digest = coverage_input_digest(corpus, &terms, limit);
    let started = tokio::time::Instant::now();
    let candidate_owner_paths =
        resident_fixed_string_owner_paths(corpus, &terms, limit as usize, deadline, started)?;
    Ok(RuntimeResidentLexicalReceipt {
        content_generation_digest: corpus.receipt.content_generation_digest.clone(),
        coverage_input_digest,
        candidate_owner_paths,
        elapsed_micros: started.elapsed().as_micros().try_into().unwrap_or(u64::MAX),
        process_count: 0,
    })
}

fn resident_fixed_string_owner_paths(
    corpus: &agent_semantic_search::ColdRgCorpusArtifact,
    terms: &BTreeSet<String>,
    limit: usize,
    deadline: Duration,
    started: tokio::time::Instant,
) -> Result<Vec<String>, String> {
    let mut owners = BTreeSet::new();
    for (line_index, line) in corpus
        .bytes
        .split(|byte| *byte == b'\n')
        .filter(|line| !line.is_empty())
        .enumerate()
    {
        if line_index % 256 == 0 && started.elapsed() >= deadline {
            return Err("resident cold lexical deadline exceeded".to_owned());
        }
        let matched = terms.iter().any(|term| {
            let needle = term.as_bytes();
            !needle.is_empty()
                && needle.len() <= line.len()
                && line.windows(needle.len()).any(|window| window == needle)
        });
        if matched {
            let line_number = line_index as u64 + 1;
            let owner =
                agent_semantic_search::owner_for_corpus_line(&corpus.owner_spans, line_number)
                    .ok_or_else(|| {
                        "resident lexical result is outside the admitted owner corpus".to_owned()
                    })?;
            owners.insert(owner.owner_path.clone());
            if owners.len() == limit {
                break;
            }
        }
    }
    Ok(owners.into_iter().collect())
}

fn coverage_input_digest(
    corpus: &agent_semantic_search::ColdRgCorpusArtifact,
    terms: &BTreeSet<String>,
    limit: u32,
) -> String {
    let mut hasher = blake3::Hasher::new();
    for value in [
        corpus.receipt.content_generation_digest.as_bytes(),
        corpus.receipt.corpus_digest.as_bytes(),
        corpus.receipt.owner_spans_digest.as_bytes(),
    ] {
        hasher.update(&(value.len() as u64).to_le_bytes());
        hasher.update(value);
    }
    for term in terms {
        hasher.update(&(term.len() as u64).to_le_bytes());
        hasher.update(term.as_bytes());
    }
    hasher.update(&limit.to_le_bytes());
    format!("blake3-256:{}", hasher.finalize().to_hex())
}

#[cfg(test)]
#[path = "../tests/unit/runtime_cold_rg.rs"]
mod tests;
