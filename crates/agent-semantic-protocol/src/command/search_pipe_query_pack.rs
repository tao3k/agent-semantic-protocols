//! Query-pack parsing facade for search pipe quality gates.

use super::search_pipe_model::Candidate;
use super::search_pipe_query_model::{ClauseCoverage, QueryClause, QueryTerm, TermRole};

pub(super) fn query_clauses(
    language_id: &str,
    query: &str,
    query_pack_descriptor: agent_semantic_search::SearchPipeQueryPackDescriptor<'_>,
) -> Vec<QueryClause> {
    agent_semantic_search::search_pipe_query_clauses(
        agent_semantic_search::SearchPipeQueryClausesRequest::new(
            agent_semantic_search::SearchPipeLanguageId::new(language_id),
            agent_semantic_search::SearchPipeQueryText::new(query),
        )
        .with_query_pack_descriptor(query_pack_descriptor),
    )
    .into_iter()
    .map(query_clause_from_search)
    .collect()
}

pub(super) fn query_clause_texts(
    language_id: &str,
    query: &str,
    query_pack_descriptor: agent_semantic_search::SearchPipeQueryPackDescriptor<'_>,
) -> Vec<String> {
    agent_semantic_search::search_pipe_query_clause_texts(
        agent_semantic_search::SearchPipeQueryClausesRequest::new(
            agent_semantic_search::SearchPipeLanguageId::new(language_id),
            agent_semantic_search::SearchPipeQueryText::new(query),
        )
        .with_query_pack_descriptor(query_pack_descriptor),
    )
}

pub(super) fn unique_query_terms(clauses: &[QueryClause]) -> Vec<QueryTerm> {
    let search_clauses = clauses
        .iter()
        .map(search_clause_from_protocol)
        .collect::<Vec<_>>();
    agent_semantic_search::search_pipe_unique_query_terms(&search_clauses)
        .into_iter()
        .map(query_term_from_search)
        .collect()
}

pub(super) fn clause_coverages(
    clauses: &[QueryClause],
    candidates: &[Candidate],
) -> Vec<ClauseCoverage> {
    let search_clauses = clauses
        .iter()
        .map(search_clause_from_protocol)
        .collect::<Vec<_>>();
    let search_candidates = candidates
        .iter()
        .map(
            |candidate| agent_semantic_search::SearchPipeQueryPackCandidate {
                path: candidate.path.clone(),
                symbol: candidate.symbol.clone(),
                text: candidate.text.clone(),
            },
        )
        .collect::<Vec<_>>();
    agent_semantic_search::search_pipe_clause_coverages(&search_clauses, &search_candidates)
        .into_iter()
        .map(|coverage| ClauseCoverage {
            id: coverage.id,
            matched: coverage.matched,
            missing: coverage.missing,
        })
        .collect()
}

pub(super) fn role_terms(terms: &[QueryTerm], role: TermRole) -> Vec<String> {
    let search_terms = terms
        .iter()
        .map(search_term_from_protocol)
        .collect::<Vec<_>>();
    agent_semantic_search::search_pipe_role_terms(&search_terms, search_role_from_protocol(role))
}

pub(super) fn next_query_pack_hint(
    descriptor: agent_semantic_search::SearchPipeQueryPackDescriptor<'_>,
    context_terms: &[String],
    owner_seed_terms: &[String],
    concept_terms: &[String],
) -> Option<String> {
    agent_semantic_search::search_pipe_next_query_pack_hint(
        descriptor,
        context_terms,
        owner_seed_terms,
        concept_terms,
    )
}

fn query_clause_from_search(clause: agent_semantic_search::SearchPipeQueryClause) -> QueryClause {
    QueryClause {
        terms: clause
            .terms
            .into_iter()
            .map(query_term_from_search)
            .collect(),
    }
}

fn query_term_from_search(term: agent_semantic_search::SearchPipeQueryTerm) -> QueryTerm {
    QueryTerm {
        raw: term.raw,
        lower: term.lower,
        role: query_role_from_search(term.role),
    }
}

fn query_role_from_search(role: agent_semantic_search::SearchPipeTermRole) -> TermRole {
    match role {
        agent_semantic_search::SearchPipeTermRole::Context => TermRole::Context,
        agent_semantic_search::SearchPipeTermRole::Concept => TermRole::Concept,
        agent_semantic_search::SearchPipeTermRole::Symbol => TermRole::Symbol,
    }
}

fn search_clause_from_protocol(
    clause: &QueryClause,
) -> agent_semantic_search::SearchPipeQueryClause {
    agent_semantic_search::SearchPipeQueryClause {
        terms: clause.terms.iter().map(search_term_from_protocol).collect(),
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
