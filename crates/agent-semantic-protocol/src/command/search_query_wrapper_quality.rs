//! Query-wrapper quality adapter for protocol DTOs.

use std::path::PathBuf;

use agent_semantic_search::{
    QueryWrapperQualityCandidate, QueryWrapperSearchClause,
    analyze_query_wrapper_quality as analyze_search_query_wrapper_quality,
};

use super::search_pipe_model::Candidate;
use super::search_query_wrapper_model::{
    QueryWrapperClause, QueryWrapperClauseCoverage, QueryWrapperQuality,
};

pub(super) fn analyze_query_wrapper_quality(
    scopes: &[PathBuf],
    clauses: &[QueryWrapperClause],
    terms: &[String],
    candidates: &[Candidate],
) -> QueryWrapperQuality {
    let search_clauses = clauses
        .iter()
        .map(|clause| QueryWrapperSearchClause {
            id: clause.id,
            terms: clause.terms.clone(),
            axis_terms: clause.axis_terms.clone(),
        })
        .collect::<Vec<_>>();
    let search_candidates = candidates
        .iter()
        .map(|candidate| QueryWrapperQualityCandidate {
            path: candidate.path.clone(),
            symbol: candidate.symbol.clone(),
            text: candidate.text.clone(),
        })
        .collect::<Vec<_>>();
    let quality =
        analyze_search_query_wrapper_quality(scopes, &search_clauses, terms, &search_candidates);
    QueryWrapperQuality {
        query_pack_quality: quality.query_pack_quality,
        scope_quality: quality.scope_quality,
        package_cohesion: quality.package_cohesion,
        packages: quality.packages,
        risks: quality.risks,
        noise: quality.noise,
        allow_query_selector: quality.allow_query_selector,
        clause_coverages: quality
            .clause_coverages
            .into_iter()
            .map(|coverage| QueryWrapperClauseCoverage {
                id: coverage.id,
                matched: coverage.matched,
                missing: coverage.missing,
            })
            .collect(),
    }
}
