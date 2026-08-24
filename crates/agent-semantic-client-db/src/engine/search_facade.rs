use crate::engine::facade::ClientDbEngine;
use crate::engine::turso_search::{
    TursoClientDbSearchDocument, TursoClientDbSearchResult,
    replace_turso_search_document_generation, search_turso_documents,
};
use agent_semantic_client_core::state_core::TURSO_BACKEND;

use crate::ClientDbBackend;

impl ClientDbEngine {
    /// Atomically replace one root-bound search projection generation.
    pub async fn replace_search_document_generation(
        &self,
        namespace: &str,
        route: &agent_semantic_search_projection::SemanticSearchRouteDecision,
        source_snapshot: &agent_semantic_content_identity::SourceSnapshotEvidence,
        documents: &[TursoClientDbSearchDocument],
    ) -> Result<usize, String> {
        self.bootstrap_active_turso().await?;
        replace_turso_search_document_generation(
            self.db_path(),
            namespace,
            route,
            source_snapshot,
            documents,
        )
        .await
    }

    /// Search one expected root-bound shallow projection generation.
    pub async fn search_documents(
        &self,
        namespace: &str,
        route: &agent_semantic_search_projection::SemanticSearchRouteDecision,
        source_snapshot: &agent_semantic_content_identity::SourceSnapshotEvidence,
        query: &str,
        limit: u32,
    ) -> Result<TursoClientDbSearchResult, String> {
        if self.backend() != ClientDbBackend::Turso {
            return Err(format!(
                "active DB Engine backend is {}, expected {}",
                self.backend().as_str(),
                TURSO_BACKEND
            ));
        }
        if !tokio::fs::try_exists(self.db_path())
            .await
            .map_err(|error| format!("inspect search database path: {error}"))?
        {
            return Err("explicit database search route has no database artifact".to_owned());
        }
        search_turso_documents(
            self.db_path(),
            namespace,
            route,
            source_snapshot,
            query,
            limit,
        )
        .await
    }
}
