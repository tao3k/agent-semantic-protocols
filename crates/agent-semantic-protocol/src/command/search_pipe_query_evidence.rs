//! Evidence classifier facade for query-pack quality gates.

use super::search_pipe_model::Candidate;
use super::search_pipe_query_model::{QueryTerm, TermRole};

pub(super) fn is_high_value_term(term: &QueryTerm) -> bool {
    agent_semantic_search::search_pipe_is_high_value_term(&search_term_from_protocol(term))
}

pub(super) fn weak_match(candidate: &Candidate, term: &QueryTerm) -> bool {
    agent_semantic_search::search_pipe_weak_match(
        &search_candidate_from_protocol(candidate),
        &search_term_from_protocol(term),
    )
}

pub(super) fn strong_match(language_id: &str, candidate: &Candidate, term: &QueryTerm) -> bool {
    agent_semantic_search::search_pipe_strong_match(
        language_id,
        &search_candidate_from_protocol(candidate),
        &search_term_from_protocol(term),
    )
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
    let search_terms = terms
        .iter()
        .map(search_term_from_protocol)
        .collect::<Vec<_>>();
    agent_semantic_search::search_pipe_high_value_missing(&search_terms, matched)
}

pub(super) fn path_exact_match(candidate: &Candidate, term: &QueryTerm) -> bool {
    agent_semantic_search::search_pipe_path_exact_match(
        &search_candidate_from_protocol(candidate),
        &search_term_from_protocol(term),
    )
}

pub(super) fn declaration_header_match(
    language_id: &str,
    candidate: &Candidate,
    term: &QueryTerm,
) -> bool {
    agent_semantic_search::search_pipe_declaration_header_match(
        language_id,
        &search_candidate_from_protocol(candidate),
        &search_term_from_protocol(term),
    )
}

pub(super) fn weak_reason(term: &QueryTerm, candidates: &[Candidate]) -> String {
    let search_candidates = candidates
        .iter()
        .map(search_candidate_from_protocol)
        .collect::<Vec<_>>();
    agent_semantic_search::search_pipe_weak_reason(
        &search_term_from_protocol(term),
        &search_candidates,
    )
}

pub(super) fn handle_paths(
    candidates: &[Candidate],
    include: impl Fn(&Candidate) -> bool,
) -> Vec<String> {
    let mut selected = Vec::new();
    for candidate in candidates {
        if include(candidate) {
            selected.push(search_candidate_from_protocol(candidate));
        }
    }
    agent_semantic_search::search_pipe_handle_paths(&selected, |_| true)
}

pub(super) fn parser_handles(
    language_id: &str,
    candidates: &[Candidate],
    terms: &[QueryTerm],
) -> Vec<String> {
    let search_candidates = candidates
        .iter()
        .map(search_candidate_from_protocol)
        .collect::<Vec<_>>();
    let search_terms = terms
        .iter()
        .map(search_term_from_protocol)
        .collect::<Vec<_>>();
    agent_semantic_search::search_pipe_parser_handles(
        language_id,
        &search_candidates,
        &search_terms,
    )
}

pub(super) fn search_overlay_handles(candidates: &[Candidate], terms: &[QueryTerm]) -> Vec<String> {
    let search_candidates = candidates
        .iter()
        .map(search_candidate_from_protocol)
        .collect::<Vec<_>>();
    let search_terms = terms
        .iter()
        .map(search_term_from_protocol)
        .collect::<Vec<_>>();
    agent_semantic_search::search_pipe_search_overlay_handles(&search_candidates, &search_terms)
}

fn search_candidate_from_protocol(
    candidate: &Candidate,
) -> agent_semantic_search::SearchPipeEvidenceCandidate {
    agent_semantic_search::SearchPipeEvidenceCandidate {
        path: candidate.path.clone(),
        line: candidate.line,
        symbol: candidate.symbol.clone(),
        text: candidate.text.clone(),
        source: candidate.source.clone(),
    }
}

fn search_term_from_protocol(term: &QueryTerm) -> agent_semantic_search::SearchPipeQueryTerm {
    agent_semantic_search::SearchPipeQueryTerm {
        raw: term.raw.clone(),
        lower: term.lower.clone(),
        role: search_role_from_protocol(term.role),
    }
}

fn search_role_from_protocol(role: TermRole) -> agent_semantic_search::SearchPipeTermRole {
    match role {
        TermRole::Context => agent_semantic_search::SearchPipeTermRole::Context,
        TermRole::Concept => agent_semantic_search::SearchPipeTermRole::Concept,
        TermRole::Symbol => agent_semantic_search::SearchPipeTermRole::Symbol,
    }
}
