use std::path::Path;

use agent_semantic_client_db::{ClientDbEngine, TursoClientDbSearchHit};
use agent_semantic_runtime::runtime_block_on_current_thread;

use crate::{
    RankedSearchCandidate, merge_search_candidates, source_index_lookup_terms,
    structural_index_hit_to_search_candidate,
};

/// Search-owned projection for Turso-backed structural-index stable hits.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TursoStructuralIndexSearchHit {
    pub document_id: String,
    pub selector: Option<String>,
    pub document: String,
}

/// Request for collecting stable Turso structural-index candidates.
pub struct TursoStructuralIndexCandidateRequest<'a> {
    pub project_root: &'a Path,
    pub query: &'a str,
    pub limit: u32,
}

/// Collect stable structural-index candidates through the DB Engine facade and
/// project them into the shared router candidate contract.
pub fn collect_turso_structural_index_ranked_candidates(
    request: TursoStructuralIndexCandidateRequest<'_>,
) -> Result<Vec<RankedSearchCandidate>, String> {
    runtime_block_on_current_thread(collect_turso_structural_index_ranked_candidates_async(
        request,
    ))?
}

/// Async implementation for collecting stable structural-index candidates.
pub async fn collect_turso_structural_index_ranked_candidates_async(
    request: TursoStructuralIndexCandidateRequest<'_>,
) -> Result<Vec<RankedSearchCandidate>, String> {
    if request.query.trim().is_empty() || request.limit == 0 {
        return Ok(Vec::new());
    }
    let engine = ClientDbEngine::resolve(request.project_root)?;
    let hits =
        search_turso_structural_index_documents(&engine, request.query, request.limit).await?;
    let terms = source_index_lookup_terms(request.query);
    Ok(merge_search_candidates(
        hits.iter()
            .map(|hit| structural_index_hit_to_search_candidate(hit, &terms))
            .collect(),
    ))
}

/// Query stable structural-index documents without exposing DB adapter rows to callers.
pub async fn search_turso_structural_index_documents(
    engine: &ClientDbEngine,
    query: &str,
    limit: u32,
) -> Result<Vec<TursoStructuralIndexSearchHit>, String> {
    let hits = engine
        .search_structural_index_documents(query, limit)
        .await?;
    Ok(hits
        .into_iter()
        .filter_map(turso_hit_to_structural_index_hit)
        .collect())
}

fn turso_hit_to_structural_index_hit(
    hit: TursoClientDbSearchHit,
) -> Option<TursoStructuralIndexSearchHit> {
    if hit.source != "stable" || !hit.document_id.starts_with("structural-index:") {
        return None;
    }
    Some(TursoStructuralIndexSearchHit {
        document_id: hit.document_id,
        selector: hit.selector,
        document: hit.document,
    })
}
