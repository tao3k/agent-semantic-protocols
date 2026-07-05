use agent_semantic_client_db::{
    ClientDbEngine, TursoClientDbOverlayDocument, TursoClientDbSearchHit,
};

/// Search-owned document for a transient Turso overlay namespace.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TursoOverlaySearchDocument {
    pub(crate) repo_id: String,
    pub(crate) workspace_id: String,
    pub(crate) session_id: String,
    pub(crate) base_generation: String,
    pub(crate) document_id: String,
    pub(crate) selector: Option<String>,
    pub(crate) document: String,
}

impl TursoOverlaySearchDocument {
    /// Create a search-owned transient overlay document.
    #[must_use]
    pub fn new(
        repo_id: impl Into<String>,
        workspace_id: impl Into<String>,
        session_id: impl Into<String>,
        base_generation: impl Into<String>,
        document_id: impl Into<String>,
        selector: Option<String>,
        document: impl Into<String>,
    ) -> Self {
        Self {
            repo_id: repo_id.into(),
            workspace_id: workspace_id.into(),
            session_id: session_id.into(),
            base_generation: base_generation.into(),
            document_id: document_id.into(),
            selector,
            document: document.into(),
        }
    }
}

/// Search-owned projection for Turso-backed dynamic overlay hits.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TursoOverlaySearchHit {
    pub document_id: String,
    pub selector: Option<String>,
    pub document: String,
}

/// Bootstrap the Turso overlay search store through the DB Engine adapter.
pub async fn bootstrap_turso_overlay_search_store(engine: &ClientDbEngine) -> Result<(), String> {
    engine.bootstrap_active_turso().await.map(|_| ())
}

/// Store one transient overlay document through the DB Engine Turso adapter.
pub async fn upsert_turso_overlay_search_document(
    engine: &ClientDbEngine,
    document: &TursoOverlaySearchDocument,
) -> Result<(), String> {
    engine
        .upsert_overlay_document(&document.clone().into())
        .await
}

/// Query transient overlay documents without exposing DB adapter rows to callers.
pub async fn search_turso_overlay_documents(
    engine: &ClientDbEngine,
    query: &str,
    limit: u32,
) -> Result<Vec<TursoOverlaySearchHit>, String> {
    let hits = engine.search_overlay_documents(query, limit).await?;
    Ok(hits
        .into_iter()
        .filter_map(turso_hit_to_overlay_hit)
        .collect())
}

fn turso_hit_to_overlay_hit(hit: TursoClientDbSearchHit) -> Option<TursoOverlaySearchHit> {
    if hit.source() != "overlay" {
        return None;
    }
    Some(TursoOverlaySearchHit {
        document_id: hit.document_id().to_string(),
        selector: hit.selector().map(ToString::to_string),
        document: hit.document().to_string(),
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
