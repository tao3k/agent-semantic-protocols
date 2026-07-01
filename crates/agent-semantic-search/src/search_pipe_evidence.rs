use std::collections::BTreeSet;

use crate::{SearchPipeQueryTerm, SearchPipeTermRole, search_pipe_package_key};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SearchPipeEvidenceCandidate {
    pub path: String,
    pub line: usize,
    pub symbol: String,
    pub text: String,
    pub source: String,
}

#[must_use]
pub fn search_pipe_is_high_value_term(term: &SearchPipeQueryTerm) -> bool {
    matches!(term.role, SearchPipeTermRole::Symbol)
}

#[must_use]
pub fn search_pipe_weak_match(
    candidate: &SearchPipeEvidenceCandidate,
    term: &SearchPipeQueryTerm,
) -> bool {
    candidate.symbol.to_ascii_lowercase().contains(&term.lower)
        || candidate.path.to_ascii_lowercase().contains(&term.lower)
        || candidate.text.to_ascii_lowercase().contains(&term.lower)
}

#[must_use]
pub fn search_pipe_strong_match(
    language_id: &str,
    candidate: &SearchPipeEvidenceCandidate,
    term: &SearchPipeQueryTerm,
) -> bool {
    !matches!(term.role, SearchPipeTermRole::Context)
        && (search_pipe_path_exact_match(candidate, term)
            || search_pipe_declaration_header_match(language_id, candidate, term)
            || rust_path_compound_match(language_id, candidate, term)
            || owner_local_symbol_exact_match(candidate, term))
}

#[must_use]
pub fn search_pipe_high_value_matches(
    terms: &[SearchPipeQueryTerm],
    candidates: &[SearchPipeEvidenceCandidate],
    matches: impl Fn(&SearchPipeEvidenceCandidate, &SearchPipeQueryTerm) -> bool,
) -> Vec<String> {
    terms
        .iter()
        .filter(|term| search_pipe_is_high_value_term(term))
        .filter(|term| candidates.iter().any(|candidate| matches(candidate, term)))
        .map(|term| term.raw.clone())
        .collect()
}

#[must_use]
pub fn search_pipe_high_value_missing(
    terms: &[SearchPipeQueryTerm],
    matched: &[String],
) -> Vec<String> {
    terms
        .iter()
        .filter(|term| search_pipe_is_high_value_term(term))
        .filter(|term| !matched.iter().any(|seen| seen == &term.raw))
        .map(|term| term.raw.clone())
        .collect()
}

#[must_use]
pub fn search_pipe_path_exact_match(
    candidate: &SearchPipeEvidenceCandidate,
    term: &SearchPipeQueryTerm,
) -> bool {
    path_stem_tokens(&candidate.path)
        .iter()
        .any(|token| token == &term.lower)
}

#[must_use]
pub fn search_pipe_declaration_header_match(
    language_id: &str,
    candidate: &SearchPipeEvidenceCandidate,
    term: &SearchPipeQueryTerm,
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

#[must_use]
pub fn search_pipe_weak_reason(
    term: &SearchPipeQueryTerm,
    candidates: &[SearchPipeEvidenceCandidate],
) -> String {
    let matched_candidates = candidates
        .iter()
        .filter(|candidate| search_pipe_weak_match(candidate, term))
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
        .map(|candidate| search_pipe_package_key(&candidate.path))
        .collect::<BTreeSet<_>>();
    if packages.len() > 1 {
        reasons.push("package-drift");
    }
    if reasons.is_empty() {
        reasons.push("lexical-match");
    }
    reasons.join("+")
}

#[must_use]
pub fn search_pipe_handle_paths(
    candidates: &[SearchPipeEvidenceCandidate],
    include: impl Fn(&SearchPipeEvidenceCandidate) -> bool,
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

#[must_use]
pub fn search_pipe_parser_handles(
    language_id: &str,
    candidates: &[SearchPipeEvidenceCandidate],
    terms: &[SearchPipeQueryTerm],
) -> Vec<String> {
    let mut seen_terms = BTreeSet::new();
    candidates
        .iter()
        .filter_map(|candidate| {
            let term = terms.iter().find(|term| {
                search_pipe_is_high_value_term(term)
                    && search_pipe_declaration_header_match(language_id, candidate, term)
            })?;
            if !seen_terms.insert(term.raw.clone()) {
                return None;
            }
            Some(format!(
                "{}@{}:{}",
                term.raw, candidate.path, candidate.line
            ))
        })
        .take(6)
        .collect()
}

#[must_use]
pub fn search_pipe_finder_handles(
    candidates: &[SearchPipeEvidenceCandidate],
    terms: &[SearchPipeQueryTerm],
) -> Vec<String> {
    terms
        .iter()
        .filter(|term| {
            candidates.iter().any(|candidate| {
                candidate.source == "finder" && search_pipe_weak_match(candidate, term)
            })
        })
        .map(|term| term.raw.clone())
        .collect()
}

fn owner_local_symbol_exact_match(
    candidate: &SearchPipeEvidenceCandidate,
    term: &SearchPipeQueryTerm,
) -> bool {
    !matches!(
        candidate.source.as_str(),
        "finder" | "finder-path" | "fd-query" | "rg-query" | "ingest"
    ) && candidate.symbol == term.raw
}

fn rust_path_compound_match(
    language_id: &str,
    candidate: &SearchPipeEvidenceCandidate,
    term: &SearchPipeQueryTerm,
) -> bool {
    if language_id != "rust" || !term.raw.contains("::") {
        return false;
    }
    let parts = term.raw.split("::").collect::<Vec<_>>();
    if parts.len() < 2 || parts.iter().any(|part| part.is_empty()) {
        return false;
    }
    let Some(member) = parts.last() else {
        return false;
    };
    let owner_parts = &parts[..parts.len() - 1];
    if !owner_parts
        .iter()
        .any(|part| owner_fragment_matches(candidate, part))
    {
        return false;
    }
    let member_term = SearchPipeQueryTerm {
        raw: (*member).to_string(),
        lower: member.to_ascii_lowercase(),
        role: SearchPipeTermRole::Concept,
    };
    search_pipe_declaration_header_match(language_id, candidate, &member_term)
}

fn owner_fragment_matches(candidate: &SearchPipeEvidenceCandidate, fragment: &str) -> bool {
    let lower = fragment.to_ascii_lowercase();
    path_stem_tokens(&candidate.path)
        .iter()
        .any(|token| token == &lower)
        || identifier_tokens(&candidate.text)
            .iter()
            .any(|token| token == fragment)
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
