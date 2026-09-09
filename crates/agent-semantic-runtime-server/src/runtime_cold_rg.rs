// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! One bounded ripgrep process over an immutable content-generation corpus.

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct RuntimeNativeRgAxisReceipt {
    pub content_generation_digest: String,
    pub candidate_owner_paths: Vec<String>,
    pub branch_candidate_owner_paths: Vec<Vec<String>>,
    pub branch_matches: Vec<Vec<RuntimeRgMatch>>,
    pub processes: Vec<agent_semantic_search::ContentBoundNativeRgProcessReceipt>,
    pub process_count: u8,
    pub truncated: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct RuntimeRgMatch {
    pub owner_path: String,
    pub owner_line: u64,
}

pub(crate) fn execute_runtime_native_rg_blocks(
    corpus: &agent_semantic_search::ColdRgCorpusArtifact,
    blocks: &[Vec<String>],
    limit: u32,
) -> Result<RuntimeNativeRgAxisReceipt, String> {
    if blocks.is_empty() || limit == 0 {
        return Err("native rg axis is outside the bounded Runtime envelope".to_owned());
    }
    let receipt = agent_semantic_search::execute_content_bound_native_rg_blocks(
        corpus,
        blocks,
        limit as usize,
    )?;
    Ok(RuntimeNativeRgAxisReceipt {
        content_generation_digest: corpus.receipt.content_generation_digest.clone(),
        candidate_owner_paths: receipt.axis.candidate_owner_paths,
        branch_candidate_owner_paths: receipt.axis.branch_candidate_owner_paths,
        processes: receipt.processes,
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

#[cfg(test)]
#[path = "../tests/unit/runtime_cold_rg.rs"]
mod tests;
