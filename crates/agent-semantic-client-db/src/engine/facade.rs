//! ASP-owned client DB engine facade.

use std::fs;
use std::path::{Path, PathBuf};

use agent_semantic_client_core::{
    AGENT_SEMANTIC_CLIENT_CACHE_MANIFEST_FILE, CacheExportMethod, CacheManifestStatus,
    ClientCacheFileHash, ClientCacheGeneration, ClientCacheManifest, LanguageId, ProviderId,
    SemanticSchemaId, SemanticSchemaVersion, project_client_cache_dir_read_only,
    state_core::{
        CLIENT_DB_FILE, ResolvedState, STATE_LAYOUT_VERSION, STATE_MANIFEST_FILE, TURSO_BACKEND,
    },
};
use serde::Serialize;
use serde_json::json;

use crate::db::{
    ClientDb, ClientDbArtifactEvent, ClientDbGenerationHit, ClientDbGenerationLookup,
    ClientDbProviderCommandSelection, ClientDbReport, ClientDbSyntaxQueryLookup,
    ClientDbSyntaxQueryReplay,
};
#[cfg(feature = "turso-backend")]
use crate::evidence_graph::{source_index_evidence_graph, structural_index_evidence_graph};
#[cfg(feature = "turso-backend")]
use crate::source_index::{ClientDbSourceIndexCandidate, ClientDbSourceIndexSourceKind};
use crate::source_index::{
    ClientDbSourceIndexCandidateLookup, ClientDbSourceIndexClientDirLookupRequest,
    ClientDbSourceIndexImport, ClientDbSourceIndexLookupResult, ClientDbSourceIndexLookupState,
    ClientDbSourceIndexProjectLookupRequest, ClientDbSourceIndexRefreshReport,
    ClientDbSourceIndexRefreshRequest, ClientDbSourceIndexScopeFile, ClientDbSourceIndexStats,
};
#[cfg(feature = "turso-backend")]
use crate::structural_index::ClientDbStructuralIndexImport;

use super::contract::{ClientDbBackend, ClientDbEngineBackend, ClientDbEngineFeatures};
use super::sqlite::SqliteClientDbEngineBackend;
#[cfg(feature = "turso-backend")]
use super::turso::bootstrap_turso_client_db;
use super::turso::{TursoClientDbEngineBackend, TursoClientDbEngineReport};
#[cfg(feature = "turso-backend")]
use super::turso::{
    TursoClientDbEvidenceGraphPersistReport, TursoClientDbGraphEntity, list_turso_graph_entities,
    persist_turso_evidence_graph,
};
#[cfg(feature = "turso-backend")]
use super::turso_route_receipt::{
    TursoClientDbRouteReceipt, list_turso_route_receipts, upsert_turso_route_receipt,
};
#[cfg(feature = "turso-backend")]
use super::turso_search::{
    TursoClientDbOverlayDocument, TursoClientDbSearchDocument, TursoClientDbSearchHit,
    search_turso_documents, upsert_turso_overlay_document, upsert_turso_search_document,
};

/// Resolved DB Engine paths and backend selection for one State Core workspace.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClientDbEngine {
    backend: ClientDbBackend,
    future_backend: &'static str,
    layout_version: &'static str,
    client_dir: PathBuf,
    db_path: PathBuf,
    manifest_path: PathBuf,
    artifact_path: PathBuf,
    repo_id: String,
    workspace_id: String,
    scope_id: String,
}

/// DB Engine diagnostic report for CLI and analyzer-facing receipts.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClientDbEngineReport {
    pub backend: &'static str,
    pub future_backend: &'static str,
    pub layout_version: &'static str,
    pub db_file_name: &'static str,
    pub schema_version: i64,
    pub durability: &'static str,
    pub features: ClientDbEngineFeatures,
    pub client_dir: PathBuf,
    pub db_path: PathBuf,
    pub manifest_path: PathBuf,
    pub artifact_path: PathBuf,
    pub repo_id: String,
    pub workspace_id: String,
    pub scope_id: String,
    pub future_backend_report: TursoClientDbEngineReport,
    pub sqlite_report: ClientDbReport,
}

/// DB Engine read session over the current control adapter.
pub struct ClientDbEngineReadSession {
    db: ClientDb,
}

/// DB Engine write session over the current control adapter.
pub struct ClientDbEngineWriteSession {
    db: ClientDb,
}

impl ClientDbEngineReadSession {
    /// Inspect the opened control adapter without exposing its concrete DB type.
    pub fn inspect(&self) -> Result<ClientDbReport, String> {
        self.db.inspect_open()
    }

    /// Return matching generation metadata using this already opened DB Engine session.
    pub fn lookup_generation_request(
        &self,
        language_id: &LanguageId,
        provider_id: &ProviderId,
        project_root: &Path,
        export_method: &CacheExportMethod,
        request_fingerprint: Option<String>,
    ) -> Result<Option<ClientDbGenerationHit>, String> {
        self.db.lookup_generation_open(&ClientDbGenerationLookup {
            db_path: self.db.path().to_path_buf(),
            language_id: language_id.clone(),
            provider_id: provider_id.clone(),
            project_root: project_root.to_path_buf(),
            export_method: export_method.clone(),
            request_fingerprint,
        })
    }

    /// Return recent matching generation metadata using this already opened DB Engine session.
    pub fn lookup_recent_generations_request(
        &self,
        language_id: &LanguageId,
        provider_id: &ProviderId,
        project_root: &Path,
        export_method: &CacheExportMethod,
        request_fingerprint: Option<String>,
        limit: u32,
    ) -> Result<Vec<ClientDbGenerationHit>, String> {
        self.db.lookup_recent_generations_open(
            &ClientDbGenerationLookup {
                db_path: self.db.path().to_path_buf(),
                language_id: language_id.clone(),
                provider_id: provider_id.clone(),
                project_root: project_root.to_path_buf(),
                export_method: export_method.clone(),
                request_fingerprint,
            },
            limit,
        )
    }

    /// Return syntax query replay rows using this already opened DB Engine session.
    pub fn lookup_syntax_query_replay_request(
        &self,
        language_id: &LanguageId,
        provider_id: &ProviderId,
        project_root: &Path,
        query_ast_fingerprint: String,
        selector: Option<String>,
    ) -> Result<Option<ClientDbSyntaxQueryReplay>, String> {
        self.db
            .lookup_syntax_query_replay_open(&ClientDbSyntaxQueryLookup {
                db_path: self.db.path().to_path_buf(),
                language_id: language_id.clone(),
                provider_id: provider_id.clone(),
                project_root: project_root.to_path_buf(),
                query_ast_fingerprint,
                selector,
            })
    }

    /// Return graph-turbo artifact events using this already opened DB Engine session.
    pub fn lookup_artifact_events(
        &self,
        since_timestamp_ms: Option<i64>,
        limit: u32,
    ) -> Result<Vec<ClientDbArtifactEvent>, String> {
        self.db
            .lookup_artifact_events_open(since_timestamp_ms, limit)
    }

    /// Return cached provider command selections through this DB Engine session.
    pub fn lookup_provider_command_selections(
        &self,
        project_root: &Path,
        context_fingerprint: &str,
    ) -> Result<Option<Vec<ClientDbProviderCommandSelection>>, String> {
        self.db
            .lookup_provider_command_selections(project_root, context_fingerprint)
    }
}

impl ClientDbEngineWriteSession {
    /// Inspect the opened control adapter without exposing its concrete DB type.
    pub fn inspect(&self) -> Result<ClientDbReport, String> {
        self.db.inspect_open()
    }

    /// Import one cache manifest through the DB Engine control adapter.
    pub fn import_manifest(&mut self, manifest: &ClientCacheManifest) -> Result<(), String> {
        self.db.import_manifest(manifest)
    }

    /// Synchronize the control adapter's generation universe before manifest writeback import.
    pub fn sync_cache_generations_for_manifest_writeback(
        &mut self,
        manifest: &ClientCacheManifest,
        status: &CacheManifestStatus,
    ) -> Result<(), String> {
        match status {
            CacheManifestStatus::Missing | CacheManifestStatus::Invalid => {
                self.db.clear_cache_generations()?;
            }
            CacheManifestStatus::Present => {
                self.db.prune_cache_generations_to_manifest(manifest)?;
            }
            CacheManifestStatus::Unavailable => {
                return Err(
                    "cache manifest is unavailable for DB Engine writeback sync".to_string()
                );
            }
        }
        Ok(())
    }

    /// Upsert artifact events through the DB Engine control adapter.
    pub fn upsert_artifact_events(
        &mut self,
        events: &[ClientDbArtifactEvent],
    ) -> Result<u32, String> {
        self.db.upsert_artifact_events(events)
    }

    /// Replace cached provider command selections through this DB Engine session.
    pub fn replace_provider_command_selections(
        &mut self,
        project_root: &Path,
        context_fingerprint: &str,
        selections: &[ClientDbProviderCommandSelection],
    ) -> Result<(), String> {
        self.db
            .replace_provider_command_selections(project_root, context_fingerprint, selections)
    }

    /// Return file hash evidence from the latest source-index generation.
    pub fn latest_source_index_file_hashes(
        &self,
        project_root: &Path,
        schema_id: &SemanticSchemaId,
        schema_version: &SemanticSchemaVersion,
    ) -> Result<Option<Vec<ClientCacheFileHash>>, String> {
        self.db
            .latest_source_index_file_hashes(project_root, schema_id, schema_version)
    }

    /// Return file-scoped source-index inputs from the latest generation.
    pub fn latest_source_index_scope_files(
        &self,
        project_root: &Path,
        schema_id: &SemanticSchemaId,
        schema_version: &SemanticSchemaVersion,
    ) -> Result<Option<Vec<ClientDbSourceIndexScopeFile>>, String> {
        self.db
            .latest_source_index_scope_files(project_root, schema_id, schema_version)
    }

    /// Return reusable source-index stats when the latest evidence is unchanged.
    pub fn reusable_source_index_generation(
        &self,
        project_root: &Path,
        schema_id: &SemanticSchemaId,
        schema_version: &SemanticSchemaVersion,
        file_hashes: &[ClientCacheFileHash],
    ) -> Result<Option<ClientDbSourceIndexStats>, String> {
        self.db.reusable_source_index_generation(
            project_root,
            schema_id,
            schema_version,
            file_hashes,
        )
    }

    /// Apply a source-index import through this DB Engine session.
    pub fn refresh_source_index_import(
        &mut self,
        request: ClientDbSourceIndexRefreshRequest,
    ) -> Result<ClientDbSourceIndexRefreshReport, String> {
        self.db.refresh_source_index_import(request)
    }

    /// Import one semantic tree-sitter query packet through the DB Engine control adapter.
    pub fn import_semantic_tree_sitter_query_packet(
        &mut self,
        generation: &ClientCacheGeneration,
        packet_bytes: &[u8],
    ) -> Result<(), String> {
        self.db
            .import_semantic_tree_sitter_query_packet(generation, packet_bytes)
            .map(|_| ())
    }

    /// Import one structural-index refresh artifact through the DB Engine control adapter.
    pub fn import_semantic_structural_index_refresh_packet(
        &mut self,
        generation: &ClientCacheGeneration,
        packet_bytes: &[u8],
    ) -> Result<(), String> {
        self.db
            .import_semantic_structural_index_refresh_packet(generation, packet_bytes)
            .map(|_| ())
    }

    /// Flush syntax query replay rows through this already opened DB Engine session.
    pub fn flush_syntax_query_rows(&mut self) -> Result<u32, String> {
        self.db.flush_syntax_query_rows_open()
    }

    /// Invalidate local generation rows for one project through this DB Engine session.
    pub fn invalidate_generations_for_project(
        &mut self,
        project_root: impl AsRef<Path>,
    ) -> Result<u32, String> {
        self.db
            .invalidate_generations_for_project_open(project_root)
    }
}

/// DB Engine receipt for projecting a source-index import into Turso read models.
#[cfg(feature = "turso-backend")]
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClientDbEngineSourceIndexReadModelReport {
    pub graph_entity_count: usize,
    pub graph_edge_count: usize,
    pub search_document_count: usize,
}

/// DB Engine receipt for projecting a structural-index import into Turso read models.
#[cfg(feature = "turso-backend")]
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClientDbEngineStructuralIndexReadModelReport {
    pub graph_entity_count: usize,
    pub graph_edge_count: usize,
    pub search_document_count: usize,
}

impl ClientDbEngine {
    /// Resolve the DB Engine from State Core and create the minimal layout.
    pub fn resolve(project_root: impl AsRef<Path>) -> Result<Self, String> {
        let state = ResolvedState::resolve(project_root)?;
        state.ensure_minimal_layout()?;
        let engine = Self::from_resolved_state(&state);
        engine.write_manifest()?;
        Ok(engine)
    }

    /// Build an engine descriptor from an already resolved State Core value.
    #[must_use]
    pub fn from_resolved_state(state: &ResolvedState) -> Self {
        let backend = active_client_db_backend();
        Self {
            backend,
            future_backend: TURSO_BACKEND,
            layout_version: STATE_LAYOUT_VERSION,
            client_dir: state.paths.client_dir.clone(),
            db_path: Self::db_path_for_client_dir(&state.paths.client_dir),
            manifest_path: state.paths.client_manifest_json.clone(),
            artifact_path: state.paths.artifacts_dir.clone(),
            repo_id: state.repo.repo_id.to_string(),
            workspace_id: state.workspace.workspace_id.to_string(),
            scope_id: state.scope_id.to_string(),
        }
    }

    /// Return the current DB Engine path below an already resolved client directory.
    #[must_use]
    pub fn db_path_for_client_dir(client_dir: impl AsRef<Path>) -> PathBuf {
        match active_client_db_backend() {
            ClientDbBackend::SqliteV1 => client_dir.as_ref().join(CLIENT_DB_FILE),
            ClientDbBackend::Turso => {
                TursoClientDbEngineBackend
                    .inspect(&client_dir.as_ref().join(CLIENT_DB_FILE))
                    .db_path
            }
        }
    }

    /// Return the SQLite v1 DB path below an already resolved client directory.
    #[must_use]
    pub fn sqlite_path_for_client_dir(client_dir: impl AsRef<Path>) -> PathBuf {
        client_dir.as_ref().join(CLIENT_DB_FILE)
    }

    /// Return the planned Turso DB path below an already resolved client directory.
    #[must_use]
    pub fn turso_path_for_client_dir(client_dir: impl AsRef<Path>) -> PathBuf {
        TursoClientDbEngineBackend
            .inspect(&Self::db_path_for_client_dir(client_dir))
            .db_path
    }

    /// Open the SQLite v1 control adapter for an already resolved client directory.
    pub fn open_or_create_client_dir(client_dir: impl AsRef<Path>) -> Result<ClientDb, String> {
        SqliteClientDbEngineBackend.open_or_create(&Self::sqlite_path_for_client_dir(client_dir))
    }

    /// Open the SQLite v1 control adapter read-only for an already resolved client directory.
    pub fn open_read_only_existing_client_dir(
        client_dir: impl AsRef<Path>,
    ) -> Result<Option<ClientDb>, String> {
        SqliteClientDbEngineBackend
            .open_read_only_existing(&Self::sqlite_path_for_client_dir(client_dir))
    }

    /// Open a DB Engine read session without exposing the concrete control adapter.
    pub fn open_read_session_client_dir(
        client_dir: impl AsRef<Path>,
    ) -> Result<Option<ClientDbEngineReadSession>, String> {
        Ok(Self::open_read_only_existing_client_dir(client_dir)?
            .map(|db| ClientDbEngineReadSession { db }))
    }

    /// Open a DB Engine write session without exposing the concrete control adapter.
    pub fn open_write_session_client_dir(
        client_dir: impl AsRef<Path>,
    ) -> Result<ClientDbEngineWriteSession, String> {
        Ok(ClientDbEngineWriteSession {
            db: Self::open_or_create_client_dir(client_dir)?,
        })
    }

    /// Return syntax query replay rows through the DB Engine control adapter.
    pub fn lookup_syntax_query_replay_from_client_dir(
        client_dir: impl AsRef<Path>,
        lookup: &ClientDbSyntaxQueryLookup,
    ) -> Result<Option<ClientDbSyntaxQueryReplay>, String> {
        Self::lookup_syntax_query_replay_request_from_client_dir(
            client_dir,
            &lookup.language_id,
            &lookup.provider_id,
            &lookup.project_root,
            lookup.query_ast_fingerprint.clone(),
            lookup.selector.clone(),
        )
    }

    /// Return syntax query replay rows for one normalized request through the DB Engine.
    pub fn lookup_syntax_query_replay_request_from_client_dir(
        client_dir: impl AsRef<Path>,
        language_id: &LanguageId,
        provider_id: &ProviderId,
        project_root: &Path,
        query_ast_fingerprint: String,
        selector: Option<String>,
    ) -> Result<Option<ClientDbSyntaxQueryReplay>, String> {
        let Some(db) = Self::open_read_only_existing_client_dir(client_dir)? else {
            return Ok(None);
        };
        let lookup = ClientDbSyntaxQueryLookup {
            db_path: db.path().to_path_buf(),
            language_id: language_id.clone(),
            provider_id: provider_id.clone(),
            project_root: project_root.to_path_buf(),
            query_ast_fingerprint,
            selector,
        };
        db.lookup_syntax_query_replay_open(&lookup)
    }

    /// Return graph-turbo artifact events through the DB Engine control adapter.
    pub fn lookup_artifact_events_from_client_dir(
        client_dir: impl AsRef<Path>,
        since_timestamp_ms: Option<i64>,
        limit: u32,
    ) -> Result<Vec<ClientDbArtifactEvent>, String> {
        let Some(session) = Self::open_read_session_client_dir(client_dir)? else {
            return Ok(Vec::new());
        };
        session.lookup_artifact_events(since_timestamp_ms, limit)
    }

    /// Upsert graph-turbo artifact events through the DB Engine control adapter.
    pub fn upsert_artifact_events_from_client_dir(
        client_dir: impl AsRef<Path>,
        events: &[ClientDbArtifactEvent],
    ) -> Result<u32, String> {
        Self::open_write_session_client_dir(client_dir)?.upsert_artifact_events(events)
    }

    /// Flush syntax query replay rows through the DB Engine control adapter.
    pub fn flush_syntax_query_rows_from_client_dir(
        client_dir: impl AsRef<Path>,
    ) -> Result<u32, String> {
        let client_dir = client_dir.as_ref();
        if !Self::sqlite_path_for_client_dir(client_dir).exists() {
            return Ok(0);
        }
        Self::open_write_session_client_dir(client_dir)?.flush_syntax_query_rows()
    }

    /// Invalidate local generation rows for one project through the DB Engine control adapter.
    pub fn invalidate_generations_for_project_from_client_dir(
        client_dir: impl AsRef<Path>,
        project_root: impl AsRef<Path>,
    ) -> Result<u32, String> {
        let client_dir = client_dir.as_ref();
        if !Self::sqlite_path_for_client_dir(client_dir).exists() {
            return Ok(0);
        }
        Self::open_write_session_client_dir(client_dir)?
            .invalidate_generations_for_project(project_root)
    }

    /// Lookup source-index candidates from one project's resolved DB Engine state.
    pub fn lookup_source_index_from_project(
        request: ClientDbSourceIndexProjectLookupRequest<'_>,
    ) -> Result<ClientDbSourceIndexLookupResult, String> {
        let client_dir = project_client_cache_dir_read_only(request.cache_project_root)?;
        Self::lookup_source_index_from_client_dir(ClientDbSourceIndexClientDirLookupRequest {
            client_dir: &client_dir,
            indexed_project_root: request.indexed_project_root,
            language_id: request.language_id,
            query_keys: request.query_keys,
            limit: request.limit,
        })
    }

    /// Lookup source-index candidates through the SQLite v1 control adapter.
    pub fn lookup_source_index_from_client_dir(
        request: ClientDbSourceIndexClientDirLookupRequest<'_>,
    ) -> Result<ClientDbSourceIndexLookupResult, String> {
        let db_path = Self::sqlite_path_for_client_dir(request.client_dir);
        let Some(db) = Self::open_read_only_existing_client_dir(request.client_dir)? else {
            return Ok(source_index_lookup_result(
                db_path,
                ClientDbSourceIndexLookupState::MissingDb,
                Vec::new(),
            ));
        };
        let lookup = db.lookup_source_index_candidates(&ClientDbSourceIndexCandidateLookup {
            project_root: request.indexed_project_root.to_path_buf(),
            language_id: request.language_id.cloned(),
            query_keys: request.query_keys,
            limit: request.limit,
        })?;
        Ok(source_index_lookup_result(
            db_path,
            lookup.state,
            lookup.candidates,
        ))
    }

    /// Inspect the SQLite v1 control adapter for an already resolved client directory.
    #[must_use]
    pub fn inspect_client_dir(client_dir: impl AsRef<Path>) -> ClientDbReport {
        SqliteClientDbEngineBackend.inspect(&Self::sqlite_path_for_client_dir(client_dir))
    }

    /// Inspect the planned Turso DB Engine backend for an already resolved client directory.
    #[must_use]
    pub fn inspect_turso_client_dir(client_dir: impl AsRef<Path>) -> TursoClientDbEngineReport {
        TursoClientDbEngineBackend.inspect(&Self::db_path_for_client_dir(client_dir))
    }

    /// Bootstrap the active Turso backend file and schema.
    #[cfg(feature = "turso-backend")]
    pub async fn bootstrap_active_turso(&self) -> Result<TursoClientDbEngineReport, String> {
        if self.backend != ClientDbBackend::Turso {
            return Err(format!(
                "active DB Engine backend is {}, expected {}",
                self.backend.as_str(),
                TURSO_BACKEND
            ));
        }
        let report = bootstrap_turso_client_db(&self.db_path).await?;
        self.write_manifest()?;
        Ok(report)
    }

    /// Persist a route receipt through the active DB Engine backend.
    #[cfg(feature = "turso-backend")]
    pub async fn upsert_route_receipt(
        &self,
        receipt: &TursoClientDbRouteReceipt,
    ) -> Result<(), String> {
        self.bootstrap_active_turso().await?;
        upsert_turso_route_receipt(&self.db_path, receipt).await
    }

    /// List route receipts through the active DB Engine backend, newest first.
    #[cfg(feature = "turso-backend")]
    pub async fn list_route_receipts(
        &self,
        session_id: Option<&str>,
        limit: u32,
    ) -> Result<Vec<TursoClientDbRouteReceipt>, String> {
        if self.backend != ClientDbBackend::Turso {
            return Err(format!(
                "active DB Engine backend is {}, expected {}",
                self.backend.as_str(),
                TURSO_BACKEND
            ));
        }
        if !self.db_path.exists() || limit == 0 {
            return Ok(Vec::new());
        }
        list_turso_route_receipts(
            &self.db_path,
            self.repo_id.as_str(),
            self.workspace_id.as_str(),
            session_id,
            limit,
        )
        .await
    }

    /// Persist one stable Turso search document through the active DB Engine backend.
    #[cfg(feature = "turso-backend")]
    pub async fn upsert_search_document(
        &self,
        document: &TursoClientDbSearchDocument,
    ) -> Result<(), String> {
        self.bootstrap_active_turso().await?;
        upsert_turso_search_document(&self.db_path, document).await
    }

    /// Persist one dynamic overlay document through the active DB Engine backend.
    #[cfg(feature = "turso-backend")]
    pub async fn upsert_overlay_document(
        &self,
        document: &TursoClientDbOverlayDocument,
    ) -> Result<(), String> {
        self.bootstrap_active_turso().await?;
        upsert_turso_overlay_document(&self.db_path, document).await
    }

    /// Search all Turso search lanes through the active DB Engine backend.
    #[cfg(feature = "turso-backend")]
    pub async fn search_documents(
        &self,
        query: &str,
        limit: u32,
    ) -> Result<Vec<TursoClientDbSearchHit>, String> {
        if self.backend != ClientDbBackend::Turso {
            return Err(format!(
                "active DB Engine backend is {}, expected {}",
                self.backend.as_str(),
                TURSO_BACKEND
            ));
        }
        if !self.db_path.exists() || query.trim().is_empty() || limit == 0 {
            return Ok(Vec::new());
        }
        search_turso_documents(&self.db_path, query, limit).await
    }

    /// Search dynamic overlay documents through the active DB Engine backend.
    #[cfg(feature = "turso-backend")]
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
    #[cfg(feature = "turso-backend")]
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

    /// Search stable structural-index documents through the active DB Engine backend.
    #[cfg(feature = "turso-backend")]
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

    /// Lookup source-index candidates from the active Turso EvidenceGraph read model.
    #[cfg(feature = "turso-backend")]
    pub async fn lookup_source_index_read_model(
        &self,
        query: &str,
        language_id: Option<&LanguageId>,
        limit: u32,
    ) -> Result<ClientDbSourceIndexLookupResult, String> {
        if self.backend != ClientDbBackend::Turso {
            return Err(format!(
                "active DB Engine backend is {}, expected {}",
                self.backend.as_str(),
                TURSO_BACKEND
            ));
        }
        lookup_source_index_read_model_at_path(self.db_path.clone(), query, language_id, limit)
            .await
    }

    /// Lookup source-index candidates from a resolved client directory's Turso read model.
    #[cfg(feature = "turso-backend")]
    pub async fn lookup_source_index_read_model_from_client_dir(
        client_dir: impl AsRef<Path>,
        query: &str,
        language_id: Option<&LanguageId>,
        limit: u32,
    ) -> Result<ClientDbSourceIndexLookupResult, String> {
        lookup_source_index_read_model_at_path(
            Self::turso_path_for_client_dir(client_dir),
            query,
            language_id,
            limit,
        )
        .await
    }

    /// Persist stable source-index graph and search documents through the active DB Engine backend.
    #[cfg(feature = "turso-backend")]
    pub async fn persist_source_index_read_model(
        &self,
        import: &ClientDbSourceIndexImport,
    ) -> Result<ClientDbEngineSourceIndexReadModelReport, String> {
        self.bootstrap_active_turso().await?;
        let graph = source_index_evidence_graph(import);
        let graph_report = persist_turso_evidence_graph(&self.db_path, &graph).await?;
        let search_document_count = self
            .persist_source_index_search_documents(import.generation_id.as_str(), &graph)
            .await?;
        Ok(source_index_read_model_report(
            graph_report,
            search_document_count,
        ))
    }

    /// Persist stable structural-index graph facts through the active DB Engine backend.
    #[cfg(feature = "turso-backend")]
    pub async fn persist_structural_index_read_model(
        &self,
        import: &ClientDbStructuralIndexImport,
    ) -> Result<ClientDbEngineStructuralIndexReadModelReport, String> {
        self.bootstrap_active_turso().await?;
        let graph = structural_index_evidence_graph(import);
        let graph_report = persist_turso_evidence_graph(&self.db_path, &graph).await?;
        let search_document_count = self
            .persist_structural_index_search_documents(import.generation_id.as_str(), &graph)
            .await?;
        Ok(structural_index_read_model_report(
            graph_report,
            search_document_count,
        ))
    }

    /// Return the DB manifest path below an already resolved client directory.
    #[must_use]
    pub fn manifest_path_for_client_dir(client_dir: impl AsRef<Path>) -> PathBuf {
        client_dir.as_ref().join(STATE_MANIFEST_FILE)
    }

    /// Write the DB Engine-owned client manifest for the active backend.
    pub fn write_manifest(&self) -> Result<(), String> {
        fs::create_dir_all(&self.client_dir)
            .map_err(|error| format!("create DB Engine client dir: {error}"))?;
        let report = self.inspect();
        let manifest = json!({
            "layoutVersion": report.layout_version,
            "backend": report.backend,
            "futureBackend": report.future_backend,
            "repoId": report.repo_id,
            "workspaceId": report.workspace_id,
            "scopeId": report.scope_id,
            "dbFileName": report.db_file_name,
            "schemaVersion": report.schema_version,
            "durability": report.durability,
            "features": report.features,
            "clientDir": report.client_dir,
            "dbPath": report.db_path,
            "artifactPath": report.artifact_path,
            "manifestPath": report.manifest_path,
            "generationManifestPath": self
                .client_dir
                .join(AGENT_SEMANTIC_CLIENT_CACHE_MANIFEST_FILE),
            "sqliteControlDbPath": self.sqlite_control_path(),
            "futureBackendReport": report.future_backend_report,
            "sqliteReport": report.sqlite_report,
        });
        let encoded = serde_json::to_vec_pretty(&manifest)
            .map_err(|error| format!("encode DB Engine manifest: {error}"))?;
        fs::write(&self.manifest_path, encoded)
            .map_err(|error| format!("write DB Engine manifest: {error}"))
    }

    /// Open the SQLite v1 control adapter and run idempotent schema migration.
    pub fn open_or_create(&self) -> Result<ClientDb, String> {
        self.sqlite_backend()
            .open_or_create(&self.sqlite_control_path())
    }

    /// Open the SQLite v1 control adapter read-only when the file exists.
    pub fn open_read_only_existing(&self) -> Result<Option<ClientDb>, String> {
        self.sqlite_backend()
            .open_read_only_existing(&self.sqlite_control_path())
    }

    /// Inspect the SQLite v1 control adapter without creating a DB file.
    #[must_use]
    pub fn inspect_backend(&self) -> ClientDbReport {
        self.sqlite_backend().inspect(&self.sqlite_control_path())
    }

    /// Inspect the current engine selection and active SQLite v1 adapter.
    #[must_use]
    pub fn inspect(&self) -> ClientDbEngineReport {
        let sqlite_backend = self.sqlite_backend();
        let future_backend_report = self.turso_backend().inspect(&self.db_path);
        let (db_file_name, schema_version, durability, features) = match self.backend {
            ClientDbBackend::SqliteV1 => (
                sqlite_backend.db_file_name(),
                sqlite_backend.schema_version(),
                sqlite_backend.durability().as_str(),
                sqlite_backend.features(),
            ),
            ClientDbBackend::Turso => {
                let turso_backend = self.turso_backend();
                (
                    turso_backend.db_file_name(),
                    turso_backend.schema_version(),
                    turso_backend.durability().as_str(),
                    turso_backend.features(),
                )
            }
        };
        ClientDbEngineReport {
            backend: self.backend.as_str(),
            future_backend: self.future_backend,
            layout_version: self.layout_version,
            db_file_name,
            schema_version,
            durability,
            features,
            client_dir: self.client_dir.clone(),
            db_path: self.db_path.clone(),
            manifest_path: self.manifest_path.clone(),
            artifact_path: self.artifact_path.clone(),
            repo_id: self.repo_id.clone(),
            workspace_id: self.workspace_id.clone(),
            scope_id: self.scope_id.clone(),
            future_backend_report,
            sqlite_report: self.inspect_backend(),
        }
    }

    /// Current backend selected for this engine.
    #[must_use]
    pub fn backend(&self) -> ClientDbBackend {
        self.backend
    }

    /// Future backend recorded in the State Core manifest.
    #[must_use]
    pub fn future_backend(&self) -> &'static str {
        self.future_backend
    }

    /// State layout version backing this DB engine.
    #[must_use]
    pub fn layout_version(&self) -> &'static str {
        self.layout_version
    }

    /// Resolved v2 client directory.
    #[must_use]
    pub fn client_dir(&self) -> &Path {
        &self.client_dir
    }

    /// Resolved current DB file path.
    #[must_use]
    pub fn db_path(&self) -> &Path {
        &self.db_path
    }

    /// Resolved DB manifest path.
    #[must_use]
    pub fn manifest_path(&self) -> &Path {
        &self.manifest_path
    }

    /// Resolved artifact root paired with this engine workspace.
    #[must_use]
    pub fn artifact_path(&self) -> &Path {
        &self.artifact_path
    }

    /// Stable State Core repo identity.
    #[must_use]
    pub fn repo_id(&self) -> &str {
        &self.repo_id
    }

    /// Stable State Core workspace identity.
    #[must_use]
    pub fn workspace_id(&self) -> &str {
        &self.workspace_id
    }

    /// Stable State Core scope identity.
    #[must_use]
    pub fn scope_id(&self) -> &str {
        &self.scope_id
    }

    fn sqlite_backend(&self) -> SqliteClientDbEngineBackend {
        SqliteClientDbEngineBackend
    }

    fn turso_backend(&self) -> TursoClientDbEngineBackend {
        TursoClientDbEngineBackend
    }

    fn sqlite_control_path(&self) -> PathBuf {
        Self::sqlite_path_for_client_dir(&self.client_dir)
    }

    #[cfg(feature = "turso-backend")]
    async fn persist_source_index_search_documents(
        &self,
        generation_id: &str,
        graph: &crate::ClientDbEvidenceGraph,
    ) -> Result<usize, String> {
        let mut count = 0;
        for node in &graph.nodes {
            let mut terms = vec![node.kind.to_string(), node.label.clone()];
            if let Some(path) = &node.path {
                terms.push(path.clone());
            }
            if let Some(selector) = &node.selector {
                terms.push(selector.clone());
            }
            terms.extend(node.query_keys.iter().cloned());
            let document = TursoClientDbSearchDocument {
                namespace: "source-index".to_string(),
                document_id: format!("source-index:{generation_id}:{}", node.id),
                entity_id: node.id.clone(),
                selector: node.selector.clone(),
                document: terms.join(" "),
            };
            upsert_turso_search_document(&self.db_path, &document).await?;
            count += 1;
        }
        Ok(count)
    }

    #[cfg(feature = "turso-backend")]
    async fn persist_structural_index_search_documents(
        &self,
        generation_id: &str,
        graph: &crate::ClientDbEvidenceGraph,
    ) -> Result<usize, String> {
        let mut count = 0;
        for node in graph
            .nodes
            .iter()
            .filter(|node| matches!(node.kind, "symbol" | "dependency-usage"))
        {
            let mut terms = vec![node.kind.to_string(), node.label.clone()];
            if let Some(path) = &node.path {
                terms.push(path.clone());
            }
            if let Some(selector) = &node.selector {
                terms.push(selector.clone());
            }
            if let Some(language_id) = &node.language_id {
                terms.push(language_id.clone());
            }
            if let Some(provider_id) = &node.provider_id {
                terms.push(provider_id.clone());
            }
            terms.extend(node.query_keys.iter().cloned());
            let document = TursoClientDbSearchDocument {
                namespace: "structural-index".to_string(),
                document_id: format!("structural-index:{generation_id}:{}", node.id),
                entity_id: node.id.clone(),
                selector: node.selector.clone(),
                document: terms.join(" "),
            };
            upsert_turso_search_document(&self.db_path, &document).await?;
            count += 1;
        }
        Ok(count)
    }
}

fn active_client_db_backend() -> ClientDbBackend {
    if cfg!(feature = "turso-backend") {
        ClientDbBackend::Turso
    } else {
        ClientDbBackend::SqliteV1
    }
}

fn source_index_lookup_result(
    db_path: PathBuf,
    state: ClientDbSourceIndexLookupState,
    candidates: Vec<crate::ClientDbSourceIndexCandidate>,
) -> ClientDbSourceIndexLookupResult {
    ClientDbSourceIndexLookupResult {
        db_path,
        state,
        candidates,
    }
}

#[cfg(feature = "turso-backend")]
fn source_index_read_model_report(
    graph_report: TursoClientDbEvidenceGraphPersistReport,
    search_document_count: usize,
) -> ClientDbEngineSourceIndexReadModelReport {
    ClientDbEngineSourceIndexReadModelReport {
        graph_entity_count: graph_report.entity_count,
        graph_edge_count: graph_report.edge_count,
        search_document_count,
    }
}

#[cfg(feature = "turso-backend")]
fn structural_index_read_model_report(
    graph_report: TursoClientDbEvidenceGraphPersistReport,
    search_document_count: usize,
) -> ClientDbEngineStructuralIndexReadModelReport {
    ClientDbEngineStructuralIndexReadModelReport {
        graph_entity_count: graph_report.entity_count,
        graph_edge_count: graph_report.edge_count,
        search_document_count,
    }
}

#[cfg(feature = "turso-backend")]
async fn lookup_source_index_read_model_at_path(
    db_path: PathBuf,
    query: &str,
    language_id: Option<&LanguageId>,
    limit: u32,
) -> Result<ClientDbSourceIndexLookupResult, String> {
    if !db_path.exists() {
        return Ok(source_index_lookup_result(
            db_path,
            ClientDbSourceIndexLookupState::MissingDb,
            Vec::new(),
        ));
    }
    if limit == 0 {
        return Ok(source_index_lookup_result(
            db_path,
            ClientDbSourceIndexLookupState::Miss,
            Vec::new(),
        ));
    }
    let terms = source_index_read_model_terms(query);
    let entities = list_turso_graph_entities(&db_path, Some("source-owner"), 4096).await?;
    if entities.is_empty() {
        return Ok(source_index_lookup_result(
            db_path,
            ClientDbSourceIndexLookupState::EmptyIndex,
            Vec::new(),
        ));
    }
    let mut candidates = Vec::new();
    for entity in entities {
        if let Some(language_id) = language_id
            && entity.language_id.as_deref() != Some(language_id.as_str())
        {
            continue;
        }
        if terms.is_empty() || !source_index_entity_matches(&entity, &terms) {
            continue;
        }
        candidates.push(ClientDbSourceIndexCandidate {
            path: entity.path.unwrap_or(entity.label),
            language_id: entity.language_id.map(LanguageId::from),
            provider_id: entity.provider_id.map(ProviderId::from),
            source_kind: ClientDbSourceIndexSourceKind::Other("turso-source-index".to_string()),
            line_count: None,
            query_keys: entity.query_keys,
        });
        if candidates.len() >= limit as usize {
            break;
        }
    }
    let state = if candidates.is_empty() {
        ClientDbSourceIndexLookupState::Miss
    } else {
        ClientDbSourceIndexLookupState::Hit
    };
    Ok(source_index_lookup_result(db_path, state, candidates))
}

#[cfg(feature = "turso-backend")]
fn source_index_read_model_terms(query: &str) -> Vec<String> {
    query
        .split(|character: char| {
            !character.is_alphanumeric() && character != '_' && character != '-'
        })
        .filter(|term| !term.is_empty())
        .map(|term| term.to_ascii_lowercase())
        .collect()
}

#[cfg(feature = "turso-backend")]
fn source_index_entity_matches(entity: &TursoClientDbGraphEntity, terms: &[String]) -> bool {
    let mut haystack = String::new();
    haystack.push_str(&entity.label.to_ascii_lowercase());
    haystack.push(' ');
    if let Some(path) = &entity.path {
        haystack.push_str(&path.to_ascii_lowercase());
        haystack.push(' ');
    }
    if let Some(language_id) = &entity.language_id {
        haystack.push_str(&language_id.to_ascii_lowercase());
        haystack.push(' ');
    }
    if let Some(provider_id) = &entity.provider_id {
        haystack.push_str(&provider_id.to_ascii_lowercase());
        haystack.push(' ');
    }
    for key in &entity.query_keys {
        haystack.push_str(&key.to_ascii_lowercase());
        haystack.push(' ');
    }
    terms.iter().all(|term| haystack.contains(term))
}
