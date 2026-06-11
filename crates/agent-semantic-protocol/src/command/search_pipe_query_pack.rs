//! Query-pack parsing and per-clause coverage for search pipe quality gates.

use super::search_pipe_model::Candidate;
use super::search_pipe_query_evidence::weak_match;
use super::search_pipe_query_model::{ClauseCoverage, QueryClause, QueryTerm, TermRole};

pub(super) fn query_clauses(language_id: &str, query: &str) -> Vec<QueryClause> {
    let explicit = query
        .split('|')
        .map(str::trim)
        .filter(|clause| !clause.is_empty())
        .map(|raw_clause| QueryClause {
            terms: query_terms(language_id, raw_clause),
        })
        .filter(|clause| !clause.terms.is_empty())
        .collect::<Vec<_>>();
    if query.contains('|') {
        return explicit;
    }
    auto_query_clauses(explicit)
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
    if is_weak_natural_term(raw) {
        return TermRole::Context;
    }
    if is_owner_seed_token(raw) {
        return TermRole::Symbol;
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

fn query_terms(language_id: &str, raw_clause: &str) -> Vec<QueryTerm> {
    raw_clause
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
        })
}

fn auto_query_clauses(explicit: Vec<QueryClause>) -> Vec<QueryClause> {
    let Some(single) = explicit.first() else {
        return explicit;
    };
    if explicit.len() != 1 || single.terms.len() < 6 {
        return explicit;
    }

    let mut path_terms = Vec::new();
    let mut package_terms = Vec::new();
    let mut symbol_terms = Vec::new();
    let mut concept_terms = Vec::new();
    let mut context_terms = Vec::new();
    for term in &single.terms {
        if is_path_like_token(&term.raw) {
            path_terms.push(term.clone());
        } else if is_package_like_token(&term.raw) {
            package_terms.push(term.clone());
        } else {
            match term.role {
                TermRole::Symbol => symbol_terms.push(term.clone()),
                TermRole::Concept => concept_terms.push(term.clone()),
                TermRole::Context => context_terms.push(term.clone()),
            }
        }
    }

    let clauses = [
        path_terms,
        package_terms,
        symbol_terms,
        concept_terms,
        context_terms,
    ]
    .into_iter()
    .filter(|terms| !terms.is_empty())
    .map(|terms| QueryClause { terms })
    .collect::<Vec<_>>();
    if clauses.len() > 1 { clauses } else { explicit }
}

fn is_owner_seed_token(raw: &str) -> bool {
    is_path_like_token(raw) || is_package_like_token(raw)
}

fn is_path_like_token(raw: &str) -> bool {
    raw.contains('/') || raw.contains("::") || raw.contains('.') || raw.contains('_')
}

fn is_package_like_token(raw: &str) -> bool {
    raw.matches('-').count() >= 2 && !matches!(raw, "long-field-signatures")
}

fn is_weak_natural_term(raw: &str) -> bool {
    matches!(
        raw.to_ascii_lowercase().as_str(),
        "through"
            | "smoke"
            | "dev"
            | "dependency"
            | "dependencies"
            | "weak"
            | "natural"
            | "term"
            | "terms"
    )
}
