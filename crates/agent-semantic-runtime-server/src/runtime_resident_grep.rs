// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Bounded in-process GREP over one immutable resident corpus.

use grep_matcher::Matcher;

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
    pub resident_regex_scan_count: usize,
    pub process_count: u8,
    pub filesystem_operation_count: u8,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct RuntimeGrepMatch {
    pub owner_path: String,
    pub owner_line: u64,
    pub byte_start: usize,
    pub byte_end: usize,
}

pub(crate) fn execute_runtime_resident_grep_blocks(
    corpus: &agent_semantic_search::ResidentGrepCorpusArtifact,
    owner_count: usize,
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
    mut owner_bytes: impl FnMut(&str) -> Result<Option<std::sync::Arc<[u8]>>, String>,
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
        let roots = ResidentGrepRootIndex::new(
            analysis.search_roots.iter().map(|root| root.value.as_str()),
        );
        let line_attribution = matches!(
            analysis.output_attribution,
            agent_semantic_shell_parser::NativeRgOutputAttribution::JsonPathLine
                | agent_semantic_shell_parser::NativeRgOutputAttribution::VimgrepPathLine
                | agent_semantic_shell_parser::NativeRgOutputAttribution::PathLine
        );
        let mut owners = Vec::new();
        let mut matches = Vec::new();
        let mut resident_owner_read_count = 0_usize;
        let mut resident_regex_scan_count = 0_usize;
        let mut truncated = false;
        // The public result limit belongs after exact regex verification.
        // Candidate postings may contain false positives, so truncating them
        // at the result limit could hide a later real match. The immutable
        // generation cardinality is the complete, already-admitted bound.
        let (candidate_owner_paths, candidate_receipt) =
            candidate_owner_paths(&matcher.candidate_plan, owner_count)?;
        let candidate_owners = candidate_owner_paths
            .into_iter()
            .collect::<std::collections::BTreeSet<_>>();
        'owners: for owner_path in candidate_owners {
            if !roots.contains(&owner_path) || !matcher.path_matches(&owner_path) {
                continue;
            }
            let bytes = owner_bytes(&owner_path)?.ok_or_else(|| {
                format!("resident GREP candidate owner is absent from the corpus: {owner_path}")
            })?;
            resident_owner_read_count += 1;
            resident_regex_scan_count += 1;
            // Execute rg's compiled matcher once per candidate owner. Ordinary
            // patterns cannot consume a line terminator; explicit -U
            // multiline mode removes that restriction.
            let mut owner_lines = Vec::new();
            let mut line_cursor = 0usize;
            let mut owner_line = 1u64;
            let mut last_emitted_line = None;
            let owner_line_limit = if line_attribution {
                (limit as usize).saturating_sub(matches.len()) + 1
            } else {
                1
            };
            matcher
                .expression
                .find_iter(&bytes, |occurrence| {
                    // A zero-width match at EOF is not a physical line. This
                    // excludes an empty file and the phantom line after a
                    // terminating newline while retaining real empty lines.
                    if occurrence.start() == bytes.len() && occurrence.end() == bytes.len() {
                        return true;
                    }
                    while line_cursor < occurrence.start() {
                        if bytes[line_cursor] == b'\n' {
                            owner_line = owner_line.saturating_add(1);
                        }
                        line_cursor += 1;
                    }
                    if last_emitted_line != Some(owner_line) {
                        last_emitted_line = Some(owner_line);
                        owner_lines.push((owner_line, occurrence.start(), occurrence.end()));
                    }
                    owner_lines.len() < owner_line_limit
                })
                .map_err(|error| format!("resident GREP matcher failed: {error}"))?;
            if owner_lines.is_empty() {
                continue;
            }
            if owners.len() == limit as usize {
                truncated = true;
                break;
            }
            owners.push(owner_path.clone());
            all_owners.insert(owner_path.clone());
            for (owner_line, byte_start, byte_end) in owner_lines {
                if line_attribution {
                    if matches.len() == limit as usize {
                        truncated = true;
                        break 'owners;
                    }
                    let matched = RuntimeGrepMatch {
                        owner_path: owner_path.clone(),
                        owner_line,
                        byte_start,
                        byte_end,
                    };
                    grounding_matches.push(matched.clone());
                    matches.push(matched);
                } else {
                    // Output flags control rg-compatible rendering, not the
                    // Runtime's private parser-grounding evidence. Retain one
                    // exact line per owner so Search never expands a merely
                    // owner-level hit into every selector in that file.
                    grounding_matches.push(RuntimeGrepMatch {
                        owner_path: owner_path.clone(),
                        owner_line,
                        byte_start,
                        byte_end,
                    });
                    break;
                }
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
            resident_regex_scan_count,
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
    expression: grep_regex::RegexMatcher,
    candidate_plan: agent_semantic_search::ResidentGrepCandidatePlan,
    path_globs: Vec<ResidentGrepPathGlobRule>,
    path_glob_default: bool,
}

struct ResidentGrepPathGlobRule {
    include: bool,
    matcher: globset::GlobMatcher,
}

impl ResidentGrepMatcher {
    fn path_matches(&self, owner_path: &str) -> bool {
        self.path_globs
            .iter()
            .filter(|rule| rule.matcher.is_match(owner_path))
            .fold(self.path_glob_default, |_, rule| rule.include)
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
    let raw_patterns = analysis
        .patterns
        .iter()
        .map(|pattern| pattern.value.as_str())
        .collect::<Vec<_>>();
    if raw_patterns.is_empty() {
        return Err(format!(
            "resident GREP block has no executable pattern: blockIndex={block_index}"
        ));
    }
    let candidate_expression = raw_patterns
        .iter()
        .map(|pattern| {
            if fixed {
                regex::escape(pattern)
            } else {
                (*pattern).to_owned()
            }
        })
        .map(|pattern| format!("(?:{pattern})"))
        .collect::<Vec<_>>()
        .join("|");
    let unicode = !has_option(analysis, &["--no-unicode"]);
    let candidate_plan = agent_semantic_search::build_resident_grep_candidate_plan(
        &candidate_expression,
        ignore_case,
        unicode,
    )?;
    let multiline = has_option(analysis, &["-U", "--multiline"]);
    let mut expression_builder = grep_regex::RegexMatcherBuilder::new();
    expression_builder
        .case_insensitive(ignore_case)
        .multi_line(true)
        .dot_matches_new_line(has_option(analysis, &["--multiline-dotall"]))
        .unicode(unicode)
        .fixed_strings(fixed)
        .whole_line(has_option(analysis, &["-x", "--line-regexp"]))
        .word(has_option(analysis, &["-w", "--word-regexp"]))
        .line_terminator((!multiline).then_some(b'\n'));
    let expression = expression_builder
        .build_many(&raw_patterns)
        .map_err(|error| {
            format!(
                "resident GREP pattern compilation failed: blockIndex={block_index} error={error}"
            )
        })?;
    let path_globs = analysis
        .options
        .iter()
        .filter(|option| matches!(option.option.as_str(), "-g" | "--glob" | "--iglob"))
        .map(|option| {
            option
                .value
                .as_deref()
                .ok_or_else(|| "resident GREP glob is missing its value".to_owned())
                .and_then(|value| {
                    let (include, pattern) = value
                        .strip_prefix('!')
                        .map_or((true, value), |pattern| (false, pattern));
                    if pattern.is_empty() {
                        return Err("resident GREP exclusion glob has no pattern".to_owned());
                    }
                    let mut builder = globset::GlobBuilder::new(pattern);
                    builder.case_insensitive(option.option == "--iglob");
                    builder
                        .build()
                        .map(|glob| ResidentGrepPathGlobRule {
                            include,
                            matcher: glob.compile_matcher(),
                        })
                        .map_err(|error| format!("resident GREP glob is invalid: {error}"))
                })
        })
        .collect::<Result<Vec<_>, String>>()?;
    // rg begins outside the set when any positive glob exists; an
    // exclusion-only sequence starts from the normal searchable universe.
    // Every later matching rule overrides the earlier decision.
    let path_glob_default = !path_globs.iter().any(|rule| rule.include);
    Ok(ResidentGrepMatcher {
        expression,
        candidate_plan,
        path_globs,
        path_glob_default,
    })
}

fn validate_resident_options(
    analysis: &agent_semantic_shell_parser::NativeRgArgvAnalysis,
    block_index: usize,
) -> Result<(), String> {
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
        "-U",
        "--multiline",
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

struct ResidentGrepRootIndex {
    universal: bool,
    roots: std::collections::BTreeSet<String>,
}

impl ResidentGrepRootIndex {
    fn new<'a>(roots: impl IntoIterator<Item = &'a str>) -> Self {
        let roots = roots
            .into_iter()
            .map(|root| root.trim_end_matches('/'))
            .collect::<std::collections::BTreeSet<_>>();
        let universal = roots.contains("") || roots.contains(".");
        Self {
            universal,
            roots: roots.into_iter().map(str::to_owned).collect(),
        }
    }

    fn contains(&self, owner_path: &str) -> bool {
        if self.universal || self.roots.contains(owner_path) {
            return true;
        }
        owner_path
            .match_indices('/')
            .any(|(index, _)| self.roots.contains(&owner_path[..index]))
    }
}

fn digest(bytes: &[u8]) -> String {
    format!("blake3-256:{}", blake3::hash(bytes).to_hex())
}

#[cfg(test)]
#[path = "../tests/unit/runtime_resident_grep.rs"]
mod tests;
