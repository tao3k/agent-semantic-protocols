//! Turso search and dynamic overlay DB Engine facade methods.

use std::path::Path;

use agent_semantic_client_core::state_core::TURSO_BACKEND;

use super::contract::ClientDbBackend;
use super::facade::{ClientDbEngine, block_on_db_engine_async};
use super::turso_bootstrap::bootstrap_turso_client_db;
use super::turso_search::{
    TursoClientDbOverlayDocument, TursoClientDbSearchDocument, TursoClientDbSearchHit,
    search_turso_documents, upsert_turso_overlay_document, upsert_turso_search_documents,
};

impl ClientDbEngine {
    /// Persist one stable Turso search document through the active DB Engine backend.
    pub async fn upsert_search_document(
        &self,
        document: &TursoClientDbSearchDocument,
    ) -> Result<(), String> {
        self.bootstrap_active_turso().await?;
        upsert_turso_search_documents(self.db_path(), std::slice::from_ref(document))
            .await
            .map(|_| ())
    }

    /// Persist one dynamic overlay document through the active DB Engine backend.
    pub async fn upsert_overlay_document(
        &self,
        document: &TursoClientDbOverlayDocument,
    ) -> Result<(), String> {
        self.bootstrap_active_turso().await?;
        upsert_turso_overlay_document(self.db_path(), document).await
    }

    /// Search all Turso search lanes through the active DB Engine backend.
    pub async fn search_documents(
        &self,
        query: &str,
        limit: u32,
    ) -> Result<Vec<TursoClientDbSearchHit>, String> {
        if self.backend() != ClientDbBackend::Turso {
            return Err(format!(
                "active DB Engine backend is {}, expected {}",
                self.backend().as_str(),
                TURSO_BACKEND
            ));
        }
        if !self.db_path().exists() || query.trim().is_empty() || limit == 0 {
            return Ok(Vec::new());
        }
        search_turso_documents(self.db_path(), query, limit).await
    }

    /// Search dynamic overlay documents through the active DB Engine backend.
    pub async fn search_overlay_documents(
        &self,
        query: &str,
        limit: u32,
    ) -> Result<Vec<TursoClientDbSearchHit>, String> {
        let hits = self.search_documents(query, limit).await?;
        Ok(hits
            .into_iter()
            .filter(|hit| hit.source == "overlay")
            .take(limit as usize)
            .collect())
    }

    /// Search stable source-index documents through the active DB Engine backend.
    pub async fn search_source_index_documents(
        &self,
        query: &str,
        limit: u32,
    ) -> Result<Vec<TursoClientDbSearchHit>, String> {
        let raw_limit = limit.saturating_mul(2).max(limit);
        let hits = self.search_documents(query, raw_limit).await?;
        Ok(hits
            .into_iter()
            .filter(|hit| hit.source == "stable" && hit.document_id.starts_with("source-index:"))
            .take(limit as usize)
            .collect())
    }

    /// Search stable source-index documents from an already resolved client directory.
    pub fn search_source_index_documents_from_client_dir(
        client_dir: impl AsRef<Path>,
        query: &str,
        limit: u32,
    ) -> Result<Vec<TursoClientDbSearchHit>, String> {
        let db_path = Self::turso_path_for_client_dir(client_dir.as_ref());
        if !db_path.exists() {
            return Ok(Vec::new());
        }
        let query = query.to_string();
        block_on_db_engine_async(async move {
            bootstrap_turso_client_db(&db_path).await?;
            let raw_limit = limit.saturating_mul(2).max(limit);
            let hits = search_turso_documents(&db_path, &query, raw_limit).await?;
            Ok(hits
                .into_iter()
                .filter(|hit| {
                    hit.source == "stable" && hit.document_id.starts_with("source-index:")
                })
                .take(limit as usize)
                .collect())
        })
    }

    /// Search stable structural-index documents through the active DB Engine backend.
    pub async fn search_structural_index_documents(
        &self,
        query: &str,
        limit: u32,
    ) -> Result<Vec<TursoClientDbSearchHit>, String> {
        let raw_limit = limit.saturating_mul(2).max(limit);
        let hits = self.search_documents(query, raw_limit).await?;
        Ok(hits
            .into_iter()
            .filter(|hit| {
                hit.source == "stable" && hit.document_id.starts_with("structural-index:")
            })
            .take(limit as usize)
            .collect())
    }

    /// Search stable structural-index documents from an already resolved client directory.
    pub fn search_structural_index_documents_from_client_dir(
        client_dir: impl AsRef<Path>,
        query: &str,
        limit: u32,
    ) -> Result<Vec<TursoClientDbSearchHit>, String> {
        let db_path = Self::turso_path_for_client_dir(client_dir.as_ref());
        if !db_path.exists() {
            return Ok(Vec::new());
        }
        let query = query.to_string();
        block_on_db_engine_async(async move {
            bootstrap_turso_client_db(&db_path).await?;
            let raw_limit = limit.saturating_mul(2).max(limit);
            let hits = search_turso_documents(&db_path, &query, raw_limit).await?;
            Ok(hits
                .into_iter()
                .filter(|hit| {
                    hit.source == "stable" && hit.document_id.starts_with("structural-index:")
                })
                .take(limit as usize)
                .collect())
        })
    }
}
