use std::path::Path;

use crate::engine::facade::{ClientDbEngine, block_on_db_engine_async};
use crate::engine::turso_bootstrap::bootstrap_turso_client_db;
use crate::engine::turso_search::{
    TursoClientDbSearchDocument, TursoClientDbSearchResult, TursoClientDbSearchState,
    replace_turso_search_document_generation, search_turso_documents,
};
use agent_semantic_client_core::state_core::TURSO_BACKEND;

use crate::ClientDbBackend;

impl ClientDbEngine {
    /// Atomically replace one root-bound search projection generation.
    pub async fn replace_search_document_generation(
        &self,
        namespace: &str,
        source_snapshot: &agent_semantic_content_identity::SourceSnapshotEvidence,
        documents: &[TursoClientDbSearchDocument],
    ) -> Result<usize, String> {
        self.bootstrap_active_turso().await?;
        replace_turso_search_document_generation(
            self.db_path(),
            namespace,
            source_snapshot,
            documents,
        )
        .await
    }

    /// Search one expected root-bound projection generation.
    pub async fn search_documents(
        &self,
        namespace: &str,
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
        if !self.db_path().exists() || query.trim().is_empty() || limit == 0 {
            return Ok(TursoClientDbSearchResult {
                state: TursoClientDbSearchState::EmptyIndex,
                hits: Vec::new(),
            });
        }
        search_turso_documents(self.db_path(), namespace, source_snapshot, query, limit).await
    }

    /// Search the active source-index generation for one expected Merkle root.
    pub async fn search_source_index_documents(
        &self,
        source_snapshot: &agent_semantic_content_identity::SourceSnapshotEvidence,
        query: &str,
        limit: u32,
    ) -> Result<TursoClientDbSearchResult, String> {
        self.search_documents("source-index", source_snapshot, query, limit)
            .await
    }

    /// Search the source-index generation from an already resolved client directory.
    pub fn search_source_index_documents_from_client_dir(
        client_dir: impl AsRef<Path>,
        source_snapshot: &agent_semantic_content_identity::SourceSnapshotEvidence,
        query: &str,
        limit: u32,
    ) -> Result<TursoClientDbSearchResult, String> {
        let db_path = Self::turso_path_for_client_dir(client_dir.as_ref());
        if !db_path.exists() {
            return Ok(TursoClientDbSearchResult {
                state: TursoClientDbSearchState::EmptyIndex,
                hits: Vec::new(),
            });
        }
        let source_snapshot = source_snapshot.clone();
        let query = query.to_string();
        block_on_db_engine_async(async move {
            bootstrap_turso_client_db(&db_path).await?;
            search_turso_documents(&db_path, "source-index", &source_snapshot, &query, limit).await
        })
    }

    /// Search the active structural-index generation for one expected Merkle root.
    pub async fn search_structural_index_documents(
        &self,
        source_snapshot: &agent_semantic_content_identity::SourceSnapshotEvidence,
        query: &str,
        limit: u32,
    ) -> Result<TursoClientDbSearchResult, String> {
        self.search_documents("structural-index", source_snapshot, query, limit)
            .await
    }

    /// Search the structural-index generation from an already resolved client directory.
    pub fn search_structural_index_documents_from_client_dir(
        client_dir: impl AsRef<Path>,
        source_snapshot: &agent_semantic_content_identity::SourceSnapshotEvidence,
        query: &str,
        limit: u32,
    ) -> Result<TursoClientDbSearchResult, String> {
        let db_path = Self::turso_path_for_client_dir(client_dir.as_ref());
        if !db_path.exists() {
            return Ok(TursoClientDbSearchResult {
                state: TursoClientDbSearchState::EmptyIndex,
                hits: Vec::new(),
            });
        }
        let source_snapshot = source_snapshot.clone();
        let query = query.to_string();
        block_on_db_engine_async(async move {
            bootstrap_turso_client_db(&db_path).await?;
            search_turso_documents(
                &db_path,
                "structural-index",
                &source_snapshot,
                &query,
                limit,
            )
            .await
        })
    }
}
