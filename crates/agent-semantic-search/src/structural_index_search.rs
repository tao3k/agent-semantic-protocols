use agent_semantic_client_db::{ClientDbEngine, TursoClientDbSearchHit};

/// Search-owned projection for Turso-backed structural-index stable hits.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TursoStructuralIndexSearchHit {
    pub document_id: String,
    pub selector: Option<String>,
    pub document: String,
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
