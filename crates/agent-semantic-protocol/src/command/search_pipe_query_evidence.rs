//! Evidence classifiers for query-pack quality gates.

use std::collections::BTreeSet;

use super::search_pipe_model::Candidate;
use super::search_pipe_query_model::{QueryTerm, TermRole};

pub(super) fn is_high_value_term(term: &QueryTerm) -> bool {
    matches!(term.role, TermRole::Symbol)
}

pub(super) fn weak_match(candidate: &Candidate, term: &QueryTerm) -> bool {
    candidate.symbol.to_ascii_lowercase().contains(&term.lower)
        || candidate.path.to_ascii_lowercase().contains(&term.lower)
        || candidate.text.to_ascii_lowercase().contains(&term.lower)
}

pub(super) fn strong_match(language_id: &str, candidate: &Candidate, term: &QueryTerm) -> bool {
    !matches!(term.role, TermRole::Context)
        && (path_exact_match(candidate, term)
            || declaration_header_match(language_id, candidate, term)
            || owner_local_symbol_exact_match(candidate, term))
}

pub(super) fn high_value_matches(
    terms: &[QueryTerm],
    candidates: &[Candidate],
    matches: impl Fn(&Candidate, &QueryTerm) -> bool,
) -> Vec<String> {
    terms
        .iter()
        .filter(|term| is_high_value_term(term))
        .filter(|term| candidates.iter().any(|candidate| matches(candidate, term)))
        .map(|term| term.raw.clone())
        .collect()
}

pub(super) fn high_value_missing(terms: &[QueryTerm], matched: &[String]) -> Vec<String> {
    terms
        .iter()
        .filter(|term| is_high_value_term(term))
        .filter(|term| !matched.iter().any(|seen| seen == &term.raw))
        .map(|term| term.raw.clone())
        .collect()
}

pub(super) fn path_exact_match(candidate: &Candidate, term: &QueryTerm) -> bool {
    path_stem_tokens(&candidate.path)
        .iter()
        .any(|token| token == &term.lower)
}

pub(super) fn declaration_header_match(
    language_id: &str,
    candidate: &Candidate,
    term: &QueryTerm,
) -> bool {
    let tokens = identifier_tokens(&candidate.text);
    if !tokens.iter().any(|token| token == &term.raw) {
        return false;
    }
    let declaration_keywords = match language_id {
        "rust" => &[
            "fn", "struct", "enum", "trait", "type", "mod", "const", "static", "impl",
        ][..],
        "typescript" => &[
            "export",
            "const",
            "function",
            "class",
            "interface",
            "type",
            "namespace",
            "enum",
        ][..],
        "python" => &["def", "class"][..],
        "julia" => &["function", "struct", "module", "const"][..],
        _ => &[][..],
    };
    tokens
        .iter()
        .any(|token| declaration_keywords.iter().any(|keyword| token == keyword))
}

pub(super) fn weak_reason(term: &QueryTerm, candidates: &[Candidate]) -> String {
    let matched_candidates = candidates
        .iter()
        .filter(|candidate| weak_match(candidate, term))
        .collect::<Vec<_>>();
    let mut reasons = Vec::new();
    if matched_candidates
        .iter()
        .any(|candidate| candidate.symbol == term.lower && candidate.symbol != term.raw)
    {
        reasons.push("lowercase-symbol");
    }
    if matched_candidates.iter().any(|candidate| {
        candidate
            .text
            .split_whitespace()
            .any(|part| part.contains(&term.raw) && part != term.raw)
    }) {
        reasons.push("field-substring");
    }
    let packages = matched_candidates
        .iter()
        .map(|candidate| package_key(&candidate.path))
        .collect::<BTreeSet<_>>();
    if packages.len() > 1 {
        reasons.push("package-drift");
    }
    if reasons.is_empty() {
        reasons.push("lexical-match");
    }
    reasons.join("+")
}

pub(super) fn handle_paths(
    candidates: &[Candidate],
    include: impl Fn(&Candidate) -> bool,
) -> Vec<String> {
    let mut seen = BTreeSet::new();
    candidates
        .iter()
        .filter(|candidate| include(candidate))
        .filter_map(|candidate| {
            seen.insert(candidate.path.clone())
                .then_some(candidate.path.clone())
        })
        .take(6)
        .collect()
}

pub(super) fn parser_handles(
    language_id: &str,
    candidates: &[Candidate],
    terms: &[QueryTerm],
) -> Vec<String> {
    let mut seen_terms = BTreeSet::new();
    candidates
        .iter()
        .filter_map(|candidate| {
            let term = terms.iter().find(|term| {
                is_high_value_term(term) && declaration_header_match(language_id, candidate, term)
            })?;
            if !seen_terms.insert(term.raw.clone()) {
                return None;
            }
            let handle = format!("{}@{}:{}", term.raw, candidate.path, candidate.line);
            Some(handle)
        })
        .take(6)
        .collect()
}

pub(super) fn finder_handles(candidates: &[Candidate], terms: &[QueryTerm]) -> Vec<String> {
    terms
        .iter()
        .filter(|term| {
            candidates
                .iter()
                .any(|candidate| candidate.source == "finder" && weak_match(candidate, term))
        })
        .map(|term| term.raw.clone())
        .collect()
}

fn owner_local_symbol_exact_match(candidate: &Candidate, term: &QueryTerm) -> bool {
    !matches!(
        candidate.source.as_str(),
        "finder" | "finder-path" | "fd-query" | "rg-query" | "ingest"
    ) && candidate.symbol == term.raw
}

fn identifier_tokens(value: &str) -> Vec<String> {
    value
        .split(|character: char| !(character == '_' || character.is_ascii_alphanumeric()))
        .filter(|part| !part.is_empty())
        .map(ToOwned::to_owned)
        .collect()
}

fn path_stem_tokens(path: &str) -> Vec<String> {
    path.split('/')
        .map(|part| part.rsplit_once('.').map(|(stem, _)| stem).unwrap_or(part))
        .flat_map(|part| {
            part.split(|character: char| {
                !(character == '_' || character == '-' || character.is_ascii_alphanumeric())
            })
        })
        .filter(|part| !part.is_empty())
        .map(str::to_ascii_lowercase)
        .collect()
}

fn package_key(path: &str) -> String {
    let parts = path.split('/').collect::<Vec<_>>();
    if let Some(index) = parts.iter().position(|part| *part == "packages") {
        let end = (index + 3).min(parts.len());
        return parts[index..end].join("/");
    }
    parts
        .into_iter()
        .filter(|part| !part.is_empty() && *part != ".")
        .take(2)
        .collect::<Vec<_>>()
        .join("/")
}
