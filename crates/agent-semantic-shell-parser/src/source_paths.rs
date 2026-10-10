// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Projects source-path candidates from parser-owned command-stage evidence.

use std::collections::BTreeSet;
use std::collections::HashSet;

use crate::parse_bash_command_candidates;

/// Project path-like argv candidates from one already parsed command stage.
/// This avoids reparsing the rendered stage on compound Hook hot paths.
pub fn command_stage_source_paths(stage: &crate::CommandStage) -> Vec<String> {
    let mut candidates = stage.words().iter().skip(1).cloned().collect::<Vec<_>>();
    let embedded = embedded_literal_candidates(&candidates);
    candidates.retain(|candidate| revision_qualified_path_candidate(candidate).is_none());
    candidates.extend(embedded);
    stable_unique(&candidates)
}

/// Returns stable, de-duplicated argv candidates from parsed command stages.
///
/// `tokens` are accepted only as bounded evidence when the raw command is empty
/// or cannot be parsed; they do not form a second shell grammar.
pub fn command_source_paths(command: &str, tokens: &[String]) -> Vec<String> {
    let parsed_words = (!command.trim().is_empty())
        .then(|| parse_bash_command_candidates(command).ok())
        .flatten()
        .map(|stages| {
            stages
                .into_iter()
                .flat_map(|stage| stage.words().iter().skip(1).cloned().collect::<Vec<_>>())
                .collect::<Vec<_>>()
        });

    let mut candidates = parsed_words.unwrap_or_else(|| tokens.to_vec());
    let embedded_argv_candidates = embedded_literal_candidates(&candidates);
    candidates.retain(|candidate| revision_qualified_path_candidate(candidate).is_none());
    candidates.extend(crate::bash_parser::quoted_literal_candidates(command));
    candidates.extend(crate::bash_parser::bash_heredoc_literal_candidates(command));
    let embedded_candidates = embedded_literal_candidates(&candidates);
    candidates.extend(embedded_argv_candidates);
    candidates.extend(embedded_candidates);
    stable_unique(&candidates)
}

/// Returns stable literal candidates embedded in already parsed command tokens.
///
/// This keeps interpreter `-c` payload discovery in the command parser owner
/// while leaving language/provider classification to the caller.
pub fn embedded_literal_candidates(tokens: &[String]) -> Vec<String> {
    embedded_literal_candidates_impl(tokens)
}

fn embedded_literal_candidates_impl(tokens: &[String]) -> Vec<String> {
    const MAX_LITERAL_DEPTH: usize = 4;
    const MAX_LITERAL_CANDIDATES: usize = 256;

    let mut candidates = Vec::new();
    let mut seen = tokens.iter().cloned().collect::<HashSet<_>>();
    let mut frontier = tokens.to_vec();
    for _ in 0..MAX_LITERAL_DEPTH {
        let remaining = MAX_LITERAL_CANDIDATES.saturating_sub(candidates.len());
        let next = frontier
            .iter()
            .flat_map(|token| crate::bash_parser::quoted_literal_candidates(token))
            .filter(|candidate| seen.insert(candidate.clone()))
            .take(remaining)
            .collect::<Vec<_>>();
        if next.is_empty() {
            break;
        }
        candidates.extend(next.iter().cloned());
        frontier = next;
    }
    candidates.extend(
        tokens
            .iter()
            .filter_map(|token| revision_qualified_path_candidate(token))
            .map(str::to_string),
    );
    stable_unique(&candidates)
}

fn revision_qualified_path_candidate(token: &str) -> Option<&str> {
    let (revision, path) = token.split_once(':')?;
    if revision.is_empty()
        || path.is_empty()
        || path.starts_with("//")
        || (revision.len() == 1 && (path.starts_with('/') || path.starts_with('\\')))
    {
        return None;
    }
    Some(path)
}

/// Applies the caller-owned typed predicate to one parser-produced candidate.
///
/// Path, language, and provider classification deliberately remain outside
/// this syntax owner.
pub fn path_like_token_matches<F>(token: &str, mut visit: F) -> bool
where
    F: FnMut(&str) -> bool,
{
    visit(token)
}

fn stable_unique(candidates: &[String]) -> Vec<String> {
    let mut seen = BTreeSet::new();
    candidates
        .iter()
        .filter(|candidate| !candidate.is_empty() && seen.insert((*candidate).clone()))
        .cloned()
        .collect()
}
