//! Projects source-path candidates from parser-owned command-stage evidence.

use std::collections::BTreeSet;

use crate::parse_bash_command_candidates;

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
                .flat_map(|stage| stage.words().to_vec())
                .collect::<Vec<_>>()
        });

    stable_unique(parsed_words.as_deref().unwrap_or(tokens))
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
