// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Bounded in-process GREP over one immutable resident corpus.

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct RuntimeResidentGrepAxisReceipt {
    pub candidate_owner_paths: Vec<String>,
    pub branch_candidate_owner_paths: Vec<Vec<String>>,
    pub branch_matches: Vec<Vec<RuntimeGrepMatch>>,
    pub grounding_matches: Vec<RuntimeGrepMatch>,
    pub block_receipts: Vec<RuntimeResidentGrepBlockReceipt>,
    pub truncated: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct RuntimeResidentGrepBlockReceipt {
    pub exact_argv: Vec<String>,
    pub argv_digest: String,
    pub execution_engine: &'static str,
    pub candidate_source: &'static str,
    pub candidate_gram_count: usize,
    pub decoded_posting_count: usize,
    pub smallest_posting_count: usize,
    pub candidate_owner_count: usize,
    pub candidate_lookup_nanos: u64,
    pub resident_owner_read_count: usize,
    pub process_count: u8,
    pub filesystem_operation_count: u8,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct RuntimeGrepMatch {
    pub owner_path: String,
    pub owner_line: u64,
}

pub(crate) fn execute_runtime_resident_grep_blocks(
    corpus: &agent_semantic_search::ResidentGrepCorpusArtifact,
    blocks: &[Vec<String>],
    limit: u32,
    mut candidate_owner_paths: impl FnMut(
        &agent_semantic_search::ResidentGrepCandidatePlan,
        usize,
    ) -> Result<
        (
            Vec<String>,
            agent_semantic_search::ResidentByteCoverageQueryReceipt,
        ),
        String,
    >,
) -> Result<RuntimeResidentGrepAxisReceipt, String> {
    if blocks.is_empty() || limit == 0 {
        return Err("resident GREP axis is outside the bounded Runtime envelope".to_owned());
    }
    if corpus.receipt.owner_count != corpus.owner_spans.len() {
        return Err("resident GREP corpus receipt owner count drift".to_owned());
    }

    let mut branch_candidate_owner_paths = Vec::with_capacity(blocks.len());
    let mut branch_matches = Vec::with_capacity(blocks.len());
    let mut grounding_matches = Vec::new();
    let mut block_receipts = Vec::with_capacity(blocks.len());
    let mut all_owners = std::collections::BTreeSet::new();
    let mut any_truncated = false;
    for (block_index, block) in blocks.iter().enumerate() {
        let analysis = agent_semantic_shell_parser::analyze_native_rg_argv(block);
        if !analysis.is_admitted() {
            return Err(format!(
                "resident GREP argv is not admitted: blockIndex={block_index} diagnostics={:?}",
                analysis.diagnostics
            ));
        }
        validate_resident_options(&analysis, block_index)?;
        let matcher = compile_resident_matcher(&analysis, block_index)?;
        let roots = analysis
            .search_roots
            .iter()
            .map(|root| root.value.trim_end_matches('/'))
            .collect::<Vec<_>>();
        let line_attribution = matches!(
            analysis.output_attribution,
            agent_semantic_shell_parser::NativeRgOutputAttribution::JsonPathLine
                | agent_semantic_shell_parser::NativeRgOutputAttribution::VimgrepPathLine
                | agent_semantic_shell_parser::NativeRgOutputAttribution::PathLine
        );
        let mut owners = Vec::new();
        let mut matches = Vec::new();
        let mut resident_owner_read_count = 0_usize;
        let mut truncated = false;
        let (candidate_owner_paths, candidate_receipt) =
            candidate_owner_paths(&matcher.candidate_plan, limit as usize)?;
        let candidate_owners = candidate_owner_paths
            .into_iter()
            .collect::<std::collections::BTreeSet<_>>();
        'owners: for owner_path in candidate_owners {
            if !roots
                .iter()
                .any(|root| owner_is_within_root(&owner_path, root))
                || !matcher.path_matches(&owner_path)
            {
                continue;
            }
            let bytes = corpus.owner_bytes(&owner_path).ok_or_else(|| {
                format!("resident GREP candidate owner is absent from the corpus: {owner_path}")
            })?;
            resident_owner_read_count += 1;
            let mut owner_matches = bytes
                .split_inclusive(|byte| *byte == b'\n')
                .enumerate()
                .filter_map(|(line, bytes)| {
                    let line_bytes = bytes.strip_suffix(b"\n").unwrap_or(bytes);
                    matcher
                        .expression
                        .is_match(line_bytes)
                        .then_some(line as u64 + 1)
                })
                .peekable();
            if owner_matches.peek().is_none() {
                continue;
            }
            if owners.len() == limit as usize {
                truncated = true;
                break;
            }
            owners.push(owner_path.clone());
            all_owners.insert(owner_path.clone());
            if line_attribution {
                for owner_line in owner_matches {
                    if matches.len() == limit as usize {
                        truncated = true;
                        break 'owners;
                    }
                    let matched = RuntimeGrepMatch {
                        owner_path: owner_path.clone(),
                        owner_line,
                    };
                    grounding_matches.push(matched.clone());
                    matches.push(matched);
                }
            } else if let Some(owner_line) = owner_matches.next() {
                // Output flags control rg-compatible rendering, not the
                // Runtime's private parser-grounding evidence. Retain one
                // exact line per owner so Search never expands a merely
                // owner-level hit into every selector in that file.
                grounding_matches.push(RuntimeGrepMatch {
                    owner_path: owner_path.clone(),
                    owner_line,
                });
            }
        }
        any_truncated |= truncated;
        branch_candidate_owner_paths.push(owners);
        branch_matches.push(matches);
        block_receipts.push(RuntimeResidentGrepBlockReceipt {
            exact_argv: block.clone(),
            argv_digest: digest(
                &serde_json::to_vec(block)
                    .map_err(|error| format!("encode resident GREP argv: {error}"))?,
            ),
            execution_engine: "asp-resident-grep-v1",
            candidate_source: if matcher.candidate_plan.is_match_all() {
                "tantivy-fused-scope"
            } else {
                "trigram"
            },
            candidate_gram_count: candidate_receipt.requested_gram_count,
            decoded_posting_count: candidate_receipt.decoded_posting_count,
            smallest_posting_count: candidate_receipt.smallest_posting_count,
            candidate_owner_count: candidate_receipt.candidate_count,
            candidate_lookup_nanos: candidate_receipt.lookup_nanos,
            resident_owner_read_count,
            process_count: 0,
            filesystem_operation_count: 0,
        });
    }
    Ok(RuntimeResidentGrepAxisReceipt {
        candidate_owner_paths: all_owners.into_iter().collect(),
        branch_candidate_owner_paths,
        branch_matches,
        grounding_matches,
        block_receipts,
        truncated: any_truncated,
    })
}

struct ResidentGrepMatcher {
    expression: regex::bytes::Regex,
    candidate_plan: agent_semantic_search::ResidentGrepCandidatePlan,
    path_globs: Option<globset::GlobSet>,
}

impl ResidentGrepMatcher {
    fn path_matches(&self, owner_path: &str) -> bool {
        self.path_globs
            .as_ref()
            .is_none_or(|globs| globs.is_match(owner_path))
    }
}

fn compile_resident_matcher(
    analysis: &agent_semantic_shell_parser::NativeRgArgvAnalysis,
    block_index: usize,
) -> Result<ResidentGrepMatcher, String> {
    let fixed = has_option(analysis, &["-F", "--fixed-strings"]);
    let ignore_case = analysis
        .options
        .iter()
        .rev()
        .find_map(|option| match option.option.as_str() {
            "-i" | "--ignore-case" => Some(true),
            "-s" | "--case-sensitive" => Some(false),
            "-S" | "--smart-case" => Some(
                analysis
                    .patterns
                    .iter()
                    .all(|pattern| !pattern.value.chars().any(char::is_uppercase)),
            ),
            _ => None,
        })
        .unwrap_or(false);
    let mut patterns = analysis
        .patterns
        .iter()
        .map(|pattern| {
            Ok(if fixed {
                regex::escape(&pattern.value)
            } else {
                pattern.value.clone()
            })
        })
        .collect::<Result<Vec<_>, String>>()?;
    if patterns.is_empty() {
        return Err(format!(
            "resident GREP block has no executable pattern: blockIndex={block_index}"
        ));
    }
    if has_option(analysis, &["-w", "--word-regexp"])
        && !has_option(analysis, &["-x", "--line-regexp"])
    {
        patterns = patterns
            .into_iter()
            .map(|pattern| format!(r"(?:^|\W)(?:{pattern})(?:$|\W)"))
            .collect();
    }
    if has_option(analysis, &["-x", "--line-regexp"]) {
        patterns = patterns
            .into_iter()
            .map(|pattern| format!(r"^(?:{pattern})$"))
            .collect();
    }
    let expression = patterns
        .into_iter()
        .map(|pattern| format!("(?:{pattern})"))
        .collect::<Vec<_>>()
        .join("|");
    let unicode = !has_option(analysis, &["--no-unicode"]);
    let candidate_plan = agent_semantic_search::build_resident_grep_candidate_plan(
        &expression,
        ignore_case,
        unicode,
    )?;
    let expression = regex::bytes::RegexBuilder::new(&expression)
        .case_insensitive(ignore_case)
        .multi_line(true)
        .unicode(unicode)
        .build()
        .map_err(|error| {
            format!(
                "resident GREP pattern compilation failed: blockIndex={block_index} error={error}"
            )
        })?;
    let globs = analysis
        .options
        .iter()
        .filter(|option| matches!(option.option.as_str(), "-g" | "--glob" | "--iglob"))
        .map(|option| {
            option
                .value
                .as_deref()
                .ok_or_else(|| "resident GREP glob is missing its value".to_owned())
                .and_then(|value| {
                    if value.starts_with('!') {
                        return Err(
                            "reasonKind=resident-rg-option-not-materialized resident GREP exclusion globs are not materialized in V1".to_owned()
                        );
                    }
                    let mut builder = globset::GlobBuilder::new(value);
                    builder.case_insensitive(option.option == "--iglob");
                    builder
                        .build()
                        .map_err(|error| format!("resident GREP glob is invalid: {error}"))
                })
        })
        .collect::<Result<Vec<_>, String>>()?;
    let path_globs = if globs.is_empty() {
        None
    } else {
        let mut builder = globset::GlobSetBuilder::new();
        for glob in globs {
            builder.add(glob);
        }
        Some(
            builder
                .build()
                .map_err(|error| format!("build resident GREP glob set: {error}"))?,
        )
    };
    Ok(ResidentGrepMatcher {
        expression,
        candidate_plan,
        path_globs,
    })
}

fn validate_resident_options(
    analysis: &agent_semantic_shell_parser::NativeRgArgvAnalysis,
    block_index: usize,
) -> Result<(), String> {
    if analysis.options.iter().any(|option| {
        matches!(option.option.as_str(), "-g" | "--glob" | "--iglob")
            && option
                .value
                .as_deref()
                .is_some_and(|value| value.starts_with('!'))
    }) {
        return Err("reasonKind=resident-rg-option-not-materialized exclusion globs require qualified ordered override semantics".to_owned());
    }
    const SUPPORTED: &[&str] = &[
        "-e",
        "--regexp",
        "-F",
        "--fixed-strings",
        "-i",
        "--ignore-case",
        "-S",
        "--smart-case",
        "-s",
        "--case-sensitive",
        "-w",
        "--word-regexp",
        "-x",
        "--line-regexp",
        "-g",
        "--glob",
        "--iglob",
        "-n",
        "--line-number",
        "-H",
        "--with-filename",
        "-l",
        "--files-with-matches",
        "--json",
        "--vimgrep",
        "--heading",
        "--no-config",
        "--no-unicode",
        "--multiline-dotall",
    ];
    if let Some(option) = analysis
        .options
        .iter()
        .find(|option| !SUPPORTED.contains(&option.option.as_str()))
    {
        return Err(format!(
            "reasonKind=resident-rg-option-not-materialized resident GREP option is not materialized in V1: blockIndex={block_index} option={}",
            option.option
        ));
    }
    Ok(())
}

fn has_option(
    analysis: &agent_semantic_shell_parser::NativeRgArgvAnalysis,
    names: &[&str],
) -> bool {
    analysis
        .options
        .iter()
        .any(|option| names.contains(&option.option.as_str()))
}

fn owner_is_within_root(owner_path: &str, root: &str) -> bool {
    root.is_empty()
        || root == "."
        || owner_path == root
        || owner_path
            .strip_prefix(root)
            .is_some_and(|suffix| suffix.starts_with('/'))
}

fn digest(bytes: &[u8]) -> String {
    format!("blake3-256:{}", blake3::hash(bytes).to_hex())
}

#[cfg(test)]
#[path = "../tests/unit/runtime_resident_grep.rs"]
mod tests;
