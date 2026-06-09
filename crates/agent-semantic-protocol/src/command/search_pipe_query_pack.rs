//! Query-pack parsing and per-clause coverage for search pipe quality gates.

use super::search_pipe_model::Candidate;
use super::search_pipe_query_evidence::weak_match;
use super::search_pipe_query_model::{ClauseCoverage, QueryClause, QueryTerm, TermRole};

pub(super) fn query_clauses(language_id: &str, query: &str) -> Vec<QueryClause> {
    query
        .split('|')
        .map(str::trim)
        .filter(|clause| !clause.is_empty())
        .map(|raw_clause| QueryClause {
            terms: raw_clause
                .split(|character: char| character == ',' || character.is_whitespace())
                .map(str::trim)
                .filter(|term| !term.is_empty())
                .map(|raw| QueryTerm {
                    raw: raw.to_string(),
                    lower: raw.to_ascii_lowercase(),
                    role: term_role(language_id, raw),
                })
                .fold(Vec::new(), |mut terms, term| {
                    if !terms.iter().any(|seen: &QueryTerm| seen.raw == term.raw) {
                        terms.push(term);
                    }
                    terms
                }),
        })
        .filter(|clause| !clause.terms.is_empty())
        .collect()
}

pub(super) fn unique_query_terms(clauses: &[QueryClause]) -> Vec<QueryTerm> {
    clauses
        .iter()
        .flat_map(|clause| clause.terms.iter())
        .fold(Vec::new(), |mut terms, term| {
            if !terms.iter().any(|seen: &QueryTerm| seen.raw == term.raw) {
                terms.push(term.clone());
            }
            terms
        })
}

pub(super) fn clause_coverages(
    clauses: &[QueryClause],
    candidates: &[Candidate],
) -> Vec<ClauseCoverage> {
    clauses
        .iter()
        .enumerate()
        .map(|(index, clause)| {
            let matched = clause
                .terms
                .iter()
                .filter(|term| {
                    candidates
                        .iter()
                        .any(|candidate| weak_match(candidate, term))
                })
                .map(|term| term.lower.clone())
                .collect::<Vec<_>>();
            let missing = clause
                .terms
                .iter()
                .filter(|term| !matched.iter().any(|matched| matched == &term.lower))
                .map(|term| term.lower.clone())
                .collect::<Vec<_>>();
            ClauseCoverage {
                id: index + 1,
                matched,
                missing,
            }
        })
        .collect()
}

pub(super) fn role_terms(terms: &[QueryTerm], role: TermRole) -> Vec<String> {
    terms
        .iter()
        .filter(|term| term.role == role)
        .map(|term| term.raw.clone())
        .collect()
}

pub(super) fn next_query_pack_hint(
    context_terms: &[String],
    owner_seed_terms: &[String],
    concept_terms: &[String],
) -> Option<String> {
    if owner_seed_terms.len() < 2 {
        return None;
    }
    let mut clauses = vec![owner_seed_terms.join(" ")];
    if concept_terms
        .iter()
        .any(|term| term.eq_ignore_ascii_case("concurrency"))
    {
        clauses.push("concurrency runtime scheduling".to_string());
    } else if !concept_terms.is_empty() {
        clauses.push(concept_terms.join(" "));
    }
    if owner_seed_terms
        .iter()
        .any(|term| term.eq_ignore_ascii_case("Scope"))
    {
        clauses.push("Scope lifecycle".to_string());
    }
    if owner_seed_terms
        .iter()
        .any(|term| term.eq_ignore_ascii_case("Queue"))
        && owner_seed_terms
            .iter()
            .any(|term| term.eq_ignore_ascii_case("Stream"))
    {
        clauses.push("Queue Stream backpressure".to_string());
    }
    if clauses.len() == 1 && !context_terms.is_empty() {
        clauses.push(context_terms.join(" "));
    }
    Some(clauses.join("|"))
}

fn term_role(language_id: &str, raw: &str) -> TermRole {
    if language_id == "typescript" && matches!(raw, "Effect") {
        return TermRole::Context;
    }
    if raw
        .chars()
        .next()
        .is_some_and(|character| character.is_ascii_uppercase())
    {
        return TermRole::Symbol;
    }
    TermRole::Concept
}
