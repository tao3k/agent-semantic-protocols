use agent_semantic_client_db::{
    ClientDbEngine, TursoClientDbSearchDocument, TursoClientDbSearchHit,
};

/// Stable identity for one transient overlay search generation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TursoOverlaySearchScope {
    repo_id: String,
    workspace_id: String,
    session_id: String,
}

impl TursoOverlaySearchScope {
    #[must_use]
    pub fn new(
        repo_id: impl Into<String>,
        workspace_id: impl Into<String>,
        session_id: impl Into<String>,
    ) -> Self {
        Self {
            repo_id: repo_id.into(),
            workspace_id: workspace_id.into(),
            session_id: session_id.into(),
        }
    }

    fn namespace(&self) -> String {
        format!(
            "overlay:{}:{}:{}",
            self.repo_id, self.workspace_id, self.session_id
        )
    }
}

/// Search-owned document for a transient Turso overlay namespace.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TursoOverlaySearchDocument {
    scope: TursoOverlaySearchScope,
    base_generation: String,
    document_id: String,
    selector: Option<String>,
    document: String,
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
            scope: TursoOverlaySearchScope::new(repo_id, workspace_id, session_id),
            base_generation: base_generation.into(),
            document_id: document_id.into(),
            selector,
            document: document.into(),
        }
    }

    #[must_use]
    pub fn scope(&self) -> &TursoOverlaySearchScope {
        &self.scope
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

/// Atomically replace one snapshot-bound overlay generation.
pub async fn replace_turso_overlay_search_document_generation(
    engine: &ClientDbEngine,
    source_snapshot: &agent_semantic_content_identity::SourceSnapshotEvidence,
    documents: &[TursoOverlaySearchDocument],
) -> Result<usize, String> {
    let Some(first) = documents.first() else {
        return Ok(0);
    };
    for document in documents {
        if document.scope != first.scope {
            return Err("overlay generation documents must share one scope".to_string());
        }
        if document.base_generation != source_snapshot.root_digest {
            return Err(format!(
                "overlay base generation {} does not match snapshot root {}",
                document.base_generation, source_snapshot.root_digest
            ));
        }
    }
    let rows = documents
        .iter()
        .cloned()
        .map(TursoClientDbSearchDocument::from)
        .collect::<Vec<_>>();
    let namespace = first.scope.namespace();
    engine
        .replace_search_document_generation(&namespace, source_snapshot, &rows)
        .await
}

/// Query one snapshot-bound overlay generation without exposing DB adapter rows.
pub async fn search_turso_overlay_documents(
    engine: &ClientDbEngine,
    scope: &TursoOverlaySearchScope,
    source_snapshot: &agent_semantic_content_identity::SourceSnapshotEvidence,
    query: &str,
    limit: u32,
) -> Result<Vec<TursoOverlaySearchHit>, String> {
    let namespace = scope.namespace();
    let result = engine
        .search_documents(&namespace, source_snapshot, query, limit)
        .await?;
    Ok(result
        .hits
        .into_iter()
        .map(turso_hit_to_overlay_hit)
        .collect())
}

fn turso_hit_to_overlay_hit(hit: TursoClientDbSearchHit) -> TursoOverlaySearchHit {
    TursoOverlaySearchHit {
        document_id: hit.document_id().to_string(),
        selector: hit.selector().map(ToString::to_string),
        document: hit.document().to_string(),
    }
}

impl From<TursoOverlaySearchDocument> for TursoClientDbSearchDocument {
    fn from(document: TursoOverlaySearchDocument) -> Self {
        let entity_id = format!(
            "{}:{}",
            document.scope.namespace(),
            document.document_id.as_str()
        );
        Self {
            document_id: document.document_id,
            entity_id,
            selector: document.selector,
            document: document.document,
        }
    }
}
