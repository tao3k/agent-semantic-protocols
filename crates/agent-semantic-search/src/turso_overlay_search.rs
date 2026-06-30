use std::path::Path;

use agent_semantic_client_db::{
    TursoClientDbOverlayDocument, TursoClientDbSearchHit, bootstrap_turso_client_db,
    search_turso_documents, upsert_turso_overlay_document,
};

/// Search-owned document for a transient Turso overlay namespace.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TursoOverlaySearchDocument {
    pub repo_id: String,
    pub workspace_id: String,
    pub session_id: String,
    pub base_generation: String,
    pub document_id: String,
    pub selector: Option<String>,
    pub document: String,
}

/// Search-owned projection for Turso-backed dynamic overlay hits.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TursoOverlaySearchHit {
    pub document_id: String,
    pub selector: Option<String>,
    pub document: String,
}

/// Bootstrap the Turso overlay search store through the DB Engine adapter.
pub async fn bootstrap_turso_overlay_search_store(db_path: &Path) -> Result<(), String> {
    bootstrap_turso_client_db(db_path).await.map(|_| ())
}

/// Store one transient overlay document through the DB Engine Turso adapter.
pub async fn upsert_turso_overlay_search_document(
    db_path: &Path,
    document: &TursoOverlaySearchDocument,
) -> Result<(), String> {
    upsert_turso_overlay_document(db_path, &document.clone().into()).await
}

/// Query transient overlay documents without exposing DB adapter rows to callers.
pub async fn search_turso_overlay_documents(
    db_path: &Path,
    query: &str,
    limit: u32,
) -> Result<Vec<TursoOverlaySearchHit>, String> {
    let hits = search_turso_documents(db_path, query, limit).await?;
    Ok(hits
        .into_iter()
        .filter_map(turso_hit_to_overlay_hit)
        .collect())
}

fn turso_hit_to_overlay_hit(hit: TursoClientDbSearchHit) -> Option<TursoOverlaySearchHit> {
    if hit.source != "overlay" {
        return None;
    }
    Some(TursoOverlaySearchHit {
        document_id: hit.document_id,
        selector: hit.selector,
        document: hit.document,
    })
}

impl From<TursoOverlaySearchDocument> for TursoClientDbOverlayDocument {
    fn from(document: TursoOverlaySearchDocument) -> Self {
        Self {
            repo_id: document.repo_id,
            workspace_id: document.workspace_id,
            session_id: document.session_id,
            base_generation: document.base_generation,
            document_id: document.document_id,
            selector: document.selector,
            document: document.document,
        }
    }
}
