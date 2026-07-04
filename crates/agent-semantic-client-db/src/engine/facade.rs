//! ASP-owned client DB engine facade.

use std::fs;
use std::path::{Path, PathBuf};

use agent_semantic_client_core::{
    AGENT_SEMANTIC_CLIENT_CACHE_MANIFEST_FILE, CacheExportMethod, CacheManifestStatus,
    ClientCacheFileHash, ClientCacheGeneration, ClientCacheManifest, ClientDbStatus, LanguageId,
    ProviderId, SemanticSchemaId, SemanticSchemaVersion, project_client_cache_dir_read_only,
    state_core::{ResolvedState, STATE_LAYOUT_VERSION, STATE_MANIFEST_FILE, TURSO_BACKEND},
};
use serde::Serialize;
use serde_json::json;

use crate::evidence_graph::{source_index_evidence_graph, structural_index_evidence_graph};
use crate::source_index::{ClientDbSourceIndexCandidate, ClientDbSourceIndexSourceKind};
use crate::source_index::{
    ClientDbSourceIndexClientDirLookupRequest, ClientDbSourceIndexImport,
    ClientDbSourceIndexLookupResult, ClientDbSourceIndexLookupState,
    ClientDbSourceIndexProjectLookupRequest, ClientDbSourceIndexRefreshReport,
    ClientDbSourceIndexRefreshRequest, ClientDbSourceIndexScopeFile, ClientDbSourceIndexStats,
};
use crate::structural_index::{
    ClientDbStructuralIndexImport, parse_structural_index_packet_import,
};
use crate::types::{
    ClientDbArtifactEdge, ClientDbArtifactEvent, ClientDbArtifactGraphCompactRender,
    ClientDbArtifactRepairChainFrame, ClientDbArtifactRoot, ClientDbGenerationHit,
    ClientDbProofReceipt, ClientDbProviderCommandSelection, ClientDbReport,
    ClientDbSyntaxQueryLookup, ClientDbSyntaxQueryReplay,
};

use super::contract::{ClientDbBackend, ClientDbEngineBackend, ClientDbEngineFeatures};
use super::turso::{TursoClientDbEngineBackend, TursoClientDbEngineReport};
use super::turso::{connect_turso_client_db, turso_table_exists};
use super::turso_artifact::{lookup_turso_artifact_events, upsert_turso_artifact_events};
use super::turso_artifact_graph::{
    lookup_turso_artifact_edges, lookup_turso_proof_receipts, lookup_turso_repair_chain_frames,
    upsert_turso_artifact_edges, upsert_turso_artifact_roots, upsert_turso_proof_receipts,
    upsert_turso_repair_chain_frames,
};
use super::turso_bootstrap::bootstrap_turso_client_db;
use super::turso_cache::{
    clear_turso_cache_generations, invalidate_turso_cache_generations_for_project,
    lookup_recent_turso_cache_generations, prune_turso_cache_generations_to_manifest,
    upsert_turso_cache_generations,
};
use super::turso_evidence_graph::{
    TursoClientDbEvidenceGraphPersistReport, persist_turso_evidence_graph,
};
use super::turso_lock_policy::is_turso_lock_error;
use super::turso_provider_command::{
    lookup_turso_provider_command_selections, replace_turso_provider_command_selections,
};
use super::turso_route_receipt::{
    TursoClientDbRouteReceipt, list_turso_route_receipts, upsert_turso_route_receipt,
};
use super::turso_search::{
    TursoClientDbOverlayDocument, TursoClientDbSearchDocument, TursoClientDbSearchHit,
    search_turso_documents, upsert_turso_overlay_document, upsert_turso_search_documents,
};
use super::turso_source_index::{
    latest_turso_source_index_file_hashes, latest_turso_source_index_scope_files,
    lookup_reusable_turso_source_index_generation, refresh_turso_source_index_import,
};
use super::turso_syntax::{
    flush_turso_syntax_query_replay, lookup_turso_syntax_query_replay,
    upsert_turso_syntax_query_replay,
};

/// Resolved DB Engine paths and backend selection for one State Core workspace.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClientDbEngine {
    backend: ClientDbBackend,
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
}

/// DB Engine read session over the active Turso adapter.
pub struct ClientDbEngineReadSession {
    turso_db_path: PathBuf,
}

/// DB Engine write session over the active Turso adapter.
pub struct ClientDbEngineWriteSession {
    turso_db_path: PathBuf,
}

fn block_on_db_engine_async<T, F>(future: F) -> Result<T, String>
where
    T: Send + 'static,
    F: std::future::Future<Output = Result<T, String>> + Send + 'static,
{
    std::thread::spawn(move || {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|error| format!("failed to build DB Engine async runtime: {error}"))?;
        runtime.block_on(future)
    })
    .join()
    .map_err(|_| "DB Engine async runtime thread panicked".to_string())?
}

impl ClientDbEngineReadSession {
    /// Inspect the opened DB Engine session without exposing its concrete backend type.
    pub fn inspect(&self) -> Result<ClientDbReport, String> {
        Ok(turso_client_db_report(&self.turso_db_path))
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
        let turso_db_path = self.turso_db_path.clone();
        let language_id = language_id.clone();
        let provider_id = provider_id.clone();
        let project_root = project_root.to_path_buf();
        let export_method = export_method.clone();
        let turso_hits = block_on_db_engine_async(async move {
            lookup_recent_turso_cache_generations(
                &turso_db_path,
                &language_id,
                &provider_id,
                &project_root,
                &export_method,
                request_fingerprint.as_deref(),
                1,
            )
            .await
        })?;
        Ok(turso_hits.into_iter().next())
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
        let turso_db_path = self.turso_db_path.clone();
        let language_id = language_id.clone();
        let provider_id = provider_id.clone();
        let project_root = project_root.to_path_buf();
        let export_method = export_method.clone();
        block_on_db_engine_async(async move {
            lookup_recent_turso_cache_generations(
                &turso_db_path,
                &language_id,
                &provider_id,
                &project_root,
                &export_method,
                request_fingerprint.as_deref(),
                limit,
            )
            .await
        })
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
        let lookup = ClientDbSyntaxQueryLookup {
            db_path: self.turso_db_path.clone(),
            language_id: language_id.clone(),
            provider_id: provider_id.clone(),
            project_root: project_root.to_path_buf(),
            query_ast_fingerprint,
            selector,
        };
        block_on_db_engine_async(async move {
            lookup_turso_syntax_query_replay(&lookup.db_path, &lookup).await
        })
    }

    /// Return graph-turbo artifact events using this already opened DB Engine session.
    pub fn lookup_artifact_events(
        &self,
        since_timestamp_ms: Option<i64>,
        limit: u32,
    ) -> Result<Vec<ClientDbArtifactEvent>, String> {
        let db_path = self.turso_db_path.clone();
        block_on_db_engine_async(async move {
            lookup_turso_artifact_events(&db_path, since_timestamp_ms, limit).await
        })
    }

    /// Return cached provider command selections through this DB Engine session.
    pub fn lookup_provider_command_selections(
        &self,
        project_root: &Path,
        context_fingerprint: &str,
    ) -> Result<Option<Vec<ClientDbProviderCommandSelection>>, String> {
        let db_path = self.turso_db_path.clone();
        let project_root = project_root.to_path_buf();
        let context_fingerprint = context_fingerprint.to_string();
        block_on_db_engine_async(async move {
            lookup_turso_provider_command_selections(&db_path, &project_root, &context_fingerprint)
                .await
        })
    }
}

impl ClientDbEngineWriteSession {
    /// Inspect the opened DB Engine session without exposing its concrete backend type.
    pub fn inspect(&self) -> Result<ClientDbReport, String> {
        Ok(turso_client_db_report(&self.turso_db_path))
    }

    /// Import one cache manifest through the DB Engine control adapter.
    pub fn import_manifest(&mut self, manifest: &ClientCacheManifest) -> Result<(), String> {
        let turso_db_path = self.turso_db_path.clone();
        let manifest = manifest.clone();
        block_on_db_engine_async(async move {
            upsert_turso_cache_generations(&turso_db_path, &manifest)
                .await
                .map(|_| ())
        })
    }

    /// Synchronize the control adapter's generation universe before manifest writeback import.
    pub fn sync_cache_generations_for_manifest_writeback(
        &mut self,
        manifest: &ClientCacheManifest,
        status: &CacheManifestStatus,
    ) -> Result<(), String> {
        if matches!(status, CacheManifestStatus::Unavailable) {
            return Err("cache manifest is unavailable for DB Engine writeback sync".to_string());
        }
        let turso_db_path = self.turso_db_path.clone();
        let status = status.clone();
        let manifest = manifest.clone();
        block_on_db_engine_async(async move {
            match status {
                CacheManifestStatus::Missing | CacheManifestStatus::Invalid => {
                    clear_turso_cache_generations(&turso_db_path).await
                }
                CacheManifestStatus::Present => {
                    prune_turso_cache_generations_to_manifest(&turso_db_path, &manifest).await
                }
                CacheManifestStatus::Unavailable => Ok(()),
            }
        })
    }

    /// Upsert artifact events through the DB Engine control adapter.
    pub fn upsert_artifact_events(
        &mut self,
        events: &[ClientDbArtifactEvent],
    ) -> Result<u32, String> {
        let db_path = self.turso_db_path.clone();
        let events = events.to_vec();
        block_on_db_engine_async(
            async move { upsert_turso_artifact_events(&db_path, &events).await },
        )
    }

    /// Replace cached provider command selections through this DB Engine session.
    pub fn replace_provider_command_selections(
        &mut self,
        project_root: &Path,
        context_fingerprint: &str,
        selections: &[ClientDbProviderCommandSelection],
    ) -> Result<(), String> {
        let db_path = self.turso_db_path.clone();
        let project_root = project_root.to_path_buf();
        let context_fingerprint = context_fingerprint.to_string();
        let selections = selections.to_vec();
        block_on_db_engine_async(async move {
            replace_turso_provider_command_selections(
                &db_path,
                &project_root,
                &context_fingerprint,
                &selections,
            )
            .await
        })
    }

    /// Return file hash evidence from the latest source-index generation.
    pub fn latest_source_index_file_hashes(
        &self,
        project_root: &Path,
        schema_id: &SemanticSchemaId,
        schema_version: &SemanticSchemaVersion,
    ) -> Result<Option<Vec<ClientCacheFileHash>>, String> {
        let db_path = self.turso_db_path.clone();
        let project_root = project_root.to_path_buf();
        let schema_id = schema_id.clone();
        let schema_version = schema_version.clone();
        block_on_db_engine_async(async move {
            latest_turso_source_index_file_hashes(
                &db_path,
                &project_root,
                &schema_id,
                &schema_version,
            )
            .await
        })
    }

    /// Return file-scoped source-index inputs from the latest generation.
    pub fn latest_source_index_scope_files(
        &self,
        project_root: &Path,
        schema_id: &SemanticSchemaId,
        schema_version: &SemanticSchemaVersion,
    ) -> Result<Option<Vec<ClientDbSourceIndexScopeFile>>, String> {
        let db_path = self.turso_db_path.clone();
        let project_root = project_root.to_path_buf();
        let schema_id = schema_id.clone();
        let schema_version = schema_version.clone();
        block_on_db_engine_async(async move {
            latest_turso_source_index_scope_files(
                &db_path,
                &project_root,
                &schema_id,
                &schema_version,
            )
            .await
        })
    }

    /// Return reusable source-index stats when the latest evidence is unchanged.
    pub fn reusable_source_index_generation(
        &self,
        project_root: &Path,
        schema_id: &SemanticSchemaId,
        schema_version: &SemanticSchemaVersion,
        file_hashes: &[ClientCacheFileHash],
    ) -> Result<Option<ClientDbSourceIndexStats>, String> {
        let db_path = self.turso_db_path.clone();
        let project_root = project_root.to_path_buf();
        let schema_id = schema_id.clone();
        let schema_version = schema_version.clone();
        let file_hashes = file_hashes.to_vec();
        block_on_db_engine_async(async move {
            lookup_reusable_turso_source_index_generation(
                &db_path,
                &project_root,
                &schema_id,
                &schema_version,
                &file_hashes,
            )
            .await
        })
    }

    /// Apply a source-index import through this DB Engine session.
    pub fn refresh_source_index_import(
        &mut self,
        request: ClientDbSourceIndexRefreshRequest,
    ) -> Result<ClientDbSourceIndexRefreshReport, String> {
        let db_path = self.turso_db_path.clone();
        block_on_db_engine_async(async move {
            refresh_turso_source_index_import(&db_path, request).await
        })
    }

    /// Import one semantic tree-sitter query packet through the DB Engine control adapter.
    pub fn import_semantic_tree_sitter_query_packet(
        &mut self,
        generation: &ClientCacheGeneration,
        packet_bytes: &[u8],
    ) -> Result<(), String> {
        let turso_db_path = self.turso_db_path.clone();
        let generation = generation.clone();
        let packet_bytes = packet_bytes.to_vec();
        block_on_db_engine_async(async move {
            bootstrap_turso_client_db(&turso_db_path).await?;
            upsert_turso_syntax_query_replay(&turso_db_path, &generation, &packet_bytes).await
        })
    }

    /// Import one structural-index refresh artifact through the DB Engine control adapter.
    pub fn import_semantic_structural_index_refresh_packet(
        &mut self,
        generation: &ClientCacheGeneration,
        packet_bytes: &[u8],
    ) -> Result<(), String> {
        let import = parse_structural_index_packet_import(generation, packet_bytes)?;
        let db_path = self.turso_db_path.clone();
        block_on_db_engine_async(async move {
            persist_structural_index_read_model_at_path(&db_path, &import)
                .await
                .map(|_| ())
        })
    }

    /// Flush syntax query replay rows through this already opened DB Engine session.
    pub fn flush_syntax_query_rows(&mut self) -> Result<u32, String> {
        let turso_db_path = self.turso_db_path.clone();
        block_on_db_engine_async(async move {
            bootstrap_turso_client_db(&turso_db_path).await?;
            flush_turso_syntax_query_replay(&turso_db_path).await
        })
    }

    /// Invalidate local generation rows for one project through this DB Engine session.
    pub fn invalidate_generations_for_project(
        &mut self,
        project_root: impl AsRef<Path>,
    ) -> Result<u32, String> {
        let project_root = project_root.as_ref().to_path_buf();
        let turso_db_path = self.turso_db_path.clone();
        block_on_db_engine_async(async move {
            bootstrap_turso_client_db(&turso_db_path).await?;
            invalidate_turso_cache_generations_for_project(&turso_db_path, &project_root).await
        })
    }
}

/// DB Engine receipt for projecting a source-index import into Turso read models.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClientDbEngineSourceIndexReadModelReport {
    pub graph_entity_count: usize,
    pub graph_edge_count: usize,
    pub search_document_count: usize,
}

/// DB Engine receipt for projecting a structural-index import into Turso read models.
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
        let backend = TursoClientDbEngineBackend;
        backend
            .inspect(&client_dir.as_ref().join(backend.db_file_name()))
            .db_path
    }

    /// Return the planned Turso DB path below an already resolved client directory.
    #[must_use]
    pub fn turso_path_for_client_dir(client_dir: impl AsRef<Path>) -> PathBuf {
        Self::db_path_for_client_dir(client_dir)
    }

    /// Open a DB Engine read session without exposing the concrete control adapter.
    pub fn open_read_session_client_dir(
        client_dir: impl AsRef<Path>,
    ) -> Result<Option<ClientDbEngineReadSession>, String> {
        let client_dir = client_dir.as_ref().to_path_buf();
        let turso_db_path = Self::turso_path_for_client_dir(&client_dir);
        Ok(turso_db_path
            .exists()
            .then_some(ClientDbEngineReadSession { turso_db_path }))
    }

    /// Open a DB Engine write session without exposing the concrete control adapter.
    pub fn open_write_session_client_dir(
        client_dir: impl AsRef<Path>,
    ) -> Result<ClientDbEngineWriteSession, String> {
        let client_dir = client_dir.as_ref().to_path_buf();
        fs::create_dir_all(&client_dir).map_err(|error| {
            format!(
                "failed to create DB Engine client dir `{}`: {error}",
                client_dir.display()
            )
        })?;
        let turso_db_path = Self::turso_path_for_client_dir(&client_dir);
        Ok(ClientDbEngineWriteSession { turso_db_path })
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
        let lookup = ClientDbSyntaxQueryLookup {
            db_path: Self::turso_path_for_client_dir(client_dir.as_ref()),
            language_id: language_id.clone(),
            provider_id: provider_id.clone(),
            project_root: project_root.to_path_buf(),
            query_ast_fingerprint,
            selector,
        };
        block_on_db_engine_async(async move {
            lookup_turso_syntax_query_replay(&lookup.db_path, &lookup).await
        })
    }

    /// Return graph-turbo artifact events through the DB Engine facade.
    pub fn lookup_artifact_events_from_client_dir(
        client_dir: impl AsRef<Path>,
        since_timestamp_ms: Option<i64>,
        limit: u32,
    ) -> Result<Vec<ClientDbArtifactEvent>, String> {
        let db_path = Self::turso_path_for_client_dir(client_dir.as_ref());
        block_on_db_engine_async(async move {
            lookup_turso_artifact_events(&db_path, since_timestamp_ms, limit).await
        })
    }

    /// Upsert graph-turbo artifact events through the DB Engine facade.
    pub fn upsert_artifact_events_from_client_dir(
        client_dir: impl AsRef<Path>,
        events: &[ClientDbArtifactEvent],
    ) -> Result<u32, String> {
        let client_dir = client_dir.as_ref().to_path_buf();
        fs::create_dir_all(&client_dir).map_err(|error| {
            format!(
                "failed to create DB Engine client dir `{}`: {error}",
                client_dir.display()
            )
        })?;
        let db_path = Self::turso_path_for_client_dir(&client_dir);
        let events = events.to_vec();
        block_on_db_engine_async(
            async move { upsert_turso_artifact_events(&db_path, &events).await },
        )
    }

    /// Return cached provider command selections through the DB Engine facade.
    pub fn lookup_provider_command_selections_from_client_dir(
        client_dir: impl AsRef<Path>,
        project_root: &Path,
        context_fingerprint: &str,
    ) -> Result<Option<Vec<ClientDbProviderCommandSelection>>, String> {
        let db_path = Self::turso_path_for_client_dir(client_dir.as_ref());
        let project_root = project_root.to_path_buf();
        let context_fingerprint = context_fingerprint.to_string();
        block_on_db_engine_async(async move {
            lookup_turso_provider_command_selections(&db_path, &project_root, &context_fingerprint)
                .await
        })
    }

    /// Replace cached provider command selections through the DB Engine facade.
    pub fn replace_provider_command_selections_from_client_dir(
        client_dir: impl AsRef<Path>,
        project_root: &Path,
        context_fingerprint: &str,
        selections: &[ClientDbProviderCommandSelection],
    ) -> Result<(), String> {
        let client_dir = client_dir.as_ref().to_path_buf();
        fs::create_dir_all(&client_dir).map_err(|error| {
            format!(
                "failed to create DB Engine client dir `{}`: {error}",
                client_dir.display()
            )
        })?;
        let db_path = Self::turso_path_for_client_dir(&client_dir);
        let project_root = project_root.to_path_buf();
        let context_fingerprint = context_fingerprint.to_string();
        let selections = selections.to_vec();
        block_on_db_engine_async(async move {
            replace_turso_provider_command_selections(
                &db_path,
                &project_root,
                &context_fingerprint,
                &selections,
            )
            .await
        })
    }

    /// Import one cache manifest through the active DB Engine backend.
    pub fn import_manifest_from_client_dir(
        client_dir: impl AsRef<Path>,
        manifest: &ClientCacheManifest,
    ) -> Result<(), String> {
        let client_dir = client_dir.as_ref().to_path_buf();
        fs::create_dir_all(&client_dir).map_err(|error| {
            format!(
                "failed to create DB Engine client dir `{}`: {error}",
                client_dir.display()
            )
        })?;
        let db_path = Self::turso_path_for_client_dir(&client_dir);
        let manifest = manifest.clone();
        block_on_db_engine_async(async move {
            bootstrap_turso_client_db(&db_path).await?;
            prune_turso_cache_generations_to_manifest(&db_path, &manifest).await?;
            upsert_turso_cache_generations(&db_path, &manifest)
                .await
                .map(|_| ())
        })
    }

    /// Import one structural-index refresh packet through the active DB Engine backend.
    pub fn import_semantic_structural_index_refresh_packet_from_client_dir(
        client_dir: impl AsRef<Path>,
        generation: &ClientCacheGeneration,
        packet_bytes: &[u8],
    ) -> Result<(), String> {
        let client_dir = client_dir.as_ref().to_path_buf();
        fs::create_dir_all(&client_dir).map_err(|error| {
            format!(
                "failed to create DB Engine client dir `{}`: {error}",
                client_dir.display()
            )
        })?;
        let import = parse_structural_index_packet_import(generation, packet_bytes)?;
        let db_path = Self::turso_path_for_client_dir(&client_dir);
        block_on_db_engine_async(async move {
            persist_structural_index_read_model_at_path(&db_path, &import)
                .await
                .map(|_| ())
        })
    }

    /// Import one semantic tree-sitter query packet through the active DB Engine backend.
    pub fn import_semantic_tree_sitter_query_packet_from_client_dir(
        client_dir: impl AsRef<Path>,
        generation: &ClientCacheGeneration,
        packet_bytes: &[u8],
    ) -> Result<(), String> {
        let client_dir = client_dir.as_ref().to_path_buf();
        fs::create_dir_all(&client_dir).map_err(|error| {
            format!(
                "failed to create DB Engine client dir `{}`: {error}",
                client_dir.display()
            )
        })?;
        let db_path = Self::turso_path_for_client_dir(&client_dir);
        let generation = generation.clone();
        let packet_bytes = packet_bytes.to_vec();
        block_on_db_engine_async(async move {
            bootstrap_turso_client_db(&db_path).await?;
            upsert_turso_syntax_query_replay(&db_path, &generation, &packet_bytes).await
        })
    }

    /// Apply a source-index import through the active DB Engine backend.
    pub fn refresh_source_index_import_from_client_dir(
        client_dir: impl AsRef<Path>,
        request: ClientDbSourceIndexRefreshRequest,
    ) -> Result<ClientDbSourceIndexRefreshReport, String> {
        let client_dir = client_dir.as_ref().to_path_buf();
        fs::create_dir_all(&client_dir).map_err(|error| {
            format!(
                "failed to create DB Engine client dir `{}`: {error}",
                client_dir.display()
            )
        })?;
        let db_path = Self::turso_path_for_client_dir(&client_dir);
        block_on_db_engine_async(async move {
            refresh_turso_source_index_import(&db_path, request).await
        })
    }

    /// Flush syntax query replay rows through the DB Engine facade.
    pub fn flush_syntax_query_rows_from_client_dir(
        client_dir: impl AsRef<Path>,
    ) -> Result<u32, String> {
        let db_path = Self::turso_path_for_client_dir(client_dir.as_ref());
        if !db_path.exists() {
            return Ok(0);
        }
        block_on_db_engine_async(async move { flush_turso_syntax_query_replay(&db_path).await })
    }

    /// Invalidate local generation rows for one project through the DB Engine control adapter.
    pub fn invalidate_generations_for_project_from_client_dir(
        client_dir: impl AsRef<Path>,
        project_root: impl AsRef<Path>,
    ) -> Result<u32, String> {
        let db_path = Self::turso_path_for_client_dir(client_dir.as_ref());
        if !db_path.exists() {
            return Ok(0);
        }
        let project_root = project_root.as_ref().to_path_buf();
        block_on_db_engine_async(async move {
            bootstrap_turso_client_db(&db_path).await?;
            invalidate_turso_cache_generations_for_project(&db_path, &project_root).await
        })
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

    /// Lookup source-index candidates through the active Turso read model.
    pub fn lookup_source_index_from_client_dir(
        request: ClientDbSourceIndexClientDirLookupRequest<'_>,
    ) -> Result<ClientDbSourceIndexLookupResult, String> {
        let query = request
            .query_keys
            .iter()
            .map(|key| key.as_str())
            .collect::<Vec<_>>()
            .join(" ");
        let db_path = Self::turso_path_for_client_dir(request.client_dir);
        let language_id = request.language_id.cloned();
        let limit = request.limit;
        block_on_db_engine_async(async move {
            lookup_source_index_read_model_at_path(
                db_path,
                query.as_str(),
                language_id.as_ref(),
                limit,
            )
            .await
        })
    }

    /// Inspect the active DB Engine adapter for an already resolved client directory.
    #[must_use]
    pub fn inspect_client_dir(client_dir: impl AsRef<Path>) -> ClientDbReport {
        turso_client_db_report(&Self::turso_path_for_client_dir(client_dir))
    }

    /// Inspect the planned Turso DB Engine backend for an already resolved client directory.
    #[must_use]
    pub fn inspect_turso_client_dir(client_dir: impl AsRef<Path>) -> TursoClientDbEngineReport {
        TursoClientDbEngineBackend.inspect(&Self::db_path_for_client_dir(client_dir))
    }

    /// Bootstrap the active Turso backend file and schema.
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
    pub async fn upsert_route_receipt(
        &self,
        receipt: &TursoClientDbRouteReceipt,
    ) -> Result<(), String> {
        self.bootstrap_active_turso().await?;
        upsert_turso_route_receipt(&self.db_path, receipt).await
    }

    /// List route receipts through the active DB Engine backend, newest first.
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
    pub async fn upsert_search_document(
        &self,
        document: &TursoClientDbSearchDocument,
    ) -> Result<(), String> {
        self.bootstrap_active_turso().await?;
        upsert_turso_search_documents(&self.db_path, std::slice::from_ref(document))
            .await
            .map(|_| ())
    }

    /// Persist one dynamic overlay document through the active DB Engine backend.
    pub async fn upsert_overlay_document(
        &self,
        document: &TursoClientDbOverlayDocument,
    ) -> Result<(), String> {
        self.bootstrap_active_turso().await?;
        upsert_turso_overlay_document(&self.db_path, document).await
    }

    /// Search all Turso search lanes through the active DB Engine backend.
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

    /// Persist Merkle artifact roots through the active Turso DB Engine backend.
    pub async fn upsert_artifact_roots(
        &self,
        roots: &[ClientDbArtifactRoot],
    ) -> Result<u32, String> {
        if self.backend != ClientDbBackend::Turso {
            return Err(format!(
                "active DB Engine backend is {}, expected {}",
                self.backend.as_str(),
                TURSO_BACKEND
            ));
        }
        upsert_turso_artifact_roots(&self.db_path, roots).await
    }

    /// Persist Merkle artifact root edges through the active Turso DB Engine backend.
    pub async fn upsert_artifact_edges(
        &self,
        edges: &[ClientDbArtifactEdge],
    ) -> Result<u32, String> {
        if self.backend != ClientDbBackend::Turso {
            return Err(format!(
                "active DB Engine backend is {}, expected {}",
                self.backend.as_str(),
                TURSO_BACKEND
            ));
        }
        upsert_turso_artifact_edges(&self.db_path, edges).await
    }

    /// Persist repair-chain frames through the active Turso DB Engine backend.
    pub async fn upsert_repair_chain_frames(
        &self,
        frames: &[ClientDbArtifactRepairChainFrame],
    ) -> Result<u32, String> {
        if self.backend != ClientDbBackend::Turso {
            return Err(format!(
                "active DB Engine backend is {}, expected {}",
                self.backend.as_str(),
                TURSO_BACKEND
            ));
        }
        upsert_turso_repair_chain_frames(&self.db_path, frames).await
    }

    /// Persist compact proof receipt summaries through the active Turso DB Engine backend.
    pub async fn upsert_proof_receipts(
        &self,
        receipts: &[ClientDbProofReceipt],
    ) -> Result<u32, String> {
        if self.backend != ClientDbBackend::Turso {
            return Err(format!(
                "active DB Engine backend is {}, expected {}",
                self.backend.as_str(),
                TURSO_BACKEND
            ));
        }
        upsert_turso_proof_receipts(&self.db_path, receipts).await
    }

    /// Lookup Merkle artifact edges, optionally filtered by parent root hash.
    pub async fn lookup_artifact_edges(
        &self,
        parent_root_hash: Option<&str>,
        limit: u32,
    ) -> Result<Vec<ClientDbArtifactEdge>, String> {
        if self.backend != ClientDbBackend::Turso {
            return Err(format!(
                "active DB Engine backend is {}, expected {}",
                self.backend.as_str(),
                TURSO_BACKEND
            ));
        }
        lookup_turso_artifact_edges(&self.db_path, parent_root_hash, limit).await
    }

    /// Lookup repair-chain frames, optionally filtered by frame kind.
    pub async fn lookup_repair_chain_frames(
        &self,
        frame_kind: Option<&str>,
        limit: u32,
    ) -> Result<Vec<ClientDbArtifactRepairChainFrame>, String> {
        if self.backend != ClientDbBackend::Turso {
            return Err(format!(
                "active DB Engine backend is {}, expected {}",
                self.backend.as_str(),
                TURSO_BACKEND
            ));
        }
        lookup_turso_repair_chain_frames(&self.db_path, frame_kind, limit).await
    }

    /// Lookup compact proof receipt summaries, optionally filtered by root hash.
    pub async fn lookup_proof_receipts(
        &self,
        root_hash: Option<&str>,
        limit: u32,
    ) -> Result<Vec<ClientDbProofReceipt>, String> {
        if self.backend != ClientDbBackend::Turso {
            return Err(format!(
                "active DB Engine backend is {}, expected {}",
                self.backend.as_str(),
                TURSO_BACKEND
            ));
        }
        lookup_turso_proof_receipts(&self.db_path, root_hash, limit).await
    }

    /// Render repair-chain and proof receipts as compact agent-facing lines.
    pub async fn render_artifact_graph_compact(
        &self,
        frame_kind: Option<&str>,
        limit: u32,
    ) -> Result<ClientDbArtifactGraphCompactRender, String> {
        if self.backend != ClientDbBackend::Turso {
            return Err(format!(
                "active DB Engine backend is {}, expected {}",
                self.backend.as_str(),
                TURSO_BACKEND
            ));
        }
        let frames = lookup_turso_repair_chain_frames(&self.db_path, frame_kind, limit).await?;
        let receipts = lookup_turso_proof_receipts(&self.db_path, None, limit).await?;
        Ok(render_artifact_graph_compact_lines(&frames, &receipts))
    }

    /// Lookup source-index candidates from the active Turso EvidenceGraph read model.
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
    pub async fn persist_source_index_read_model(
        &self,
        import: &ClientDbSourceIndexImport,
    ) -> Result<ClientDbEngineSourceIndexReadModelReport, String> {
        let trace_started = std::time::Instant::now();
        let refresh = refresh_turso_source_index_import(
            &self.db_path,
            ClientDbSourceIndexRefreshRequest {
                import: import.clone(),
                file_count: import.file_hashes.len().min(u32::MAX as usize) as u32,
            },
        )
        .await?;
        db_engine_trace("source-index-refresh-read-model", trace_started);
        let graph = source_index_evidence_graph(import);
        db_engine_trace("source-index-graph-built", trace_started);
        let graph_report = TursoClientDbEvidenceGraphPersistReport {
            entity_count: graph.nodes.len(),
            edge_count: graph.edges.len(),
        };
        let search_document_count = refresh.owner_count as usize;
        Ok(source_index_read_model_report(
            graph_report,
            search_document_count,
        ))
    }

    /// Persist stable structural-index graph facts through the active DB Engine backend.
    pub async fn persist_structural_index_read_model(
        &self,
        import: &ClientDbStructuralIndexImport,
    ) -> Result<ClientDbEngineStructuralIndexReadModelReport, String> {
        persist_structural_index_read_model_at_path(&self.db_path, import).await
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
        });
        let encoded = serde_json::to_vec_pretty(&manifest)
            .map_err(|error| format!("encode DB Engine manifest: {error}"))?;
        fs::write(&self.manifest_path, encoded)
            .map_err(|error| format!("write DB Engine manifest: {error}"))
    }

    /// Inspect the active DB Engine adapter without creating a DB file.
    #[must_use]
    pub fn inspect_backend(&self) -> ClientDbReport {
        turso_client_db_report(&self.db_path)
    }

    /// Inspect the current Turso DB Engine selection.
    #[must_use]
    pub fn inspect(&self) -> ClientDbEngineReport {
        let turso_backend = self.turso_backend();
        ClientDbEngineReport {
            backend: self.backend.as_str(),
            layout_version: self.layout_version,
            db_file_name: turso_backend.db_file_name(),
            schema_version: turso_backend.schema_version(),
            durability: turso_backend.durability().as_str(),
            features: turso_backend.features(),
            client_dir: self.client_dir.clone(),
            db_path: self.db_path.clone(),
            manifest_path: self.manifest_path.clone(),
            artifact_path: self.artifact_path.clone(),
            repo_id: self.repo_id.clone(),
            workspace_id: self.workspace_id.clone(),
            scope_id: self.scope_id.clone(),
        }
    }

    /// Current backend selected for this engine.
    pub fn backend(&self) -> ClientDbBackend {
        self.backend
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

    fn turso_backend(&self) -> TursoClientDbEngineBackend {
        TursoClientDbEngineBackend
    }
}

fn render_artifact_graph_compact_lines(
    frames: &[ClientDbArtifactRepairChainFrame],
    receipts: &[ClientDbProofReceipt],
) -> ClientDbArtifactGraphCompactRender {
    let mut lines = vec![format!(
        "|artifactGraph frameCount={} proofReceiptCount={} disclosure=compact",
        frames.len(),
        receipts.len()
    )];
    for frame in frames {
        lines.push(format!(
            "|repairFrame kind={} root={} content={} parents={}",
            compact_atom(&frame.frame_kind),
            compact_root_ref(&frame.root),
            compact_hash(&frame.content_hash),
            frame.parents.len()
        ));
        for edge in &frame.parents {
            lines.push(format!(
                "|artifactEdge role={} parent={} child={} edge={}",
                compact_atom(&edge.role),
                compact_root_ref(&edge.parent),
                compact_root_ref(&edge.child),
                compact_hash(&edge.edge_hash)
            ));
        }
    }
    for receipt in receipts {
        lines.push(format!(
            "|proofReceipt id={} ok={} trust={} root={} summary=\"{}\"",
            compact_atom(&receipt.receipt_id),
            receipt.okay,
            compact_atom(&receipt.trust_level),
            compact_root_ref(&receipt.root),
            compact_text(&receipt.summary_for_agent)
        ));
    }
    ClientDbArtifactGraphCompactRender {
        frame_count: u32::try_from(frames.len()).unwrap_or(u32::MAX),
        proof_receipt_count: u32::try_from(receipts.len()).unwrap_or(u32::MAX),
        lines,
    }
}

fn compact_root_ref(root: &ClientDbArtifactRoot) -> String {
    format!(
        "{}@{}",
        compact_atom(&root.root_kind),
        compact_hash(&root.root_hash)
    )
}

fn compact_hash(hash: &crate::ClientDbArtifactHash) -> String {
    let prefix: String = hash.value.chars().take(16).collect();
    format!("{}:{}", compact_atom(&hash.algorithm), prefix)
}

fn compact_atom(value: &str) -> String {
    value
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric()
                || matches!(character, '-' | '_' | ':' | '/' | '.' | '@')
            {
                character
            } else {
                '_'
            }
        })
        .collect()
}

fn compact_text(value: &str) -> String {
    let mut compact = value.split_whitespace().collect::<Vec<_>>().join(" ");
    compact = compact.replace('"', "'");
    if compact.len() > 160 {
        compact.truncate(157);
        compact.push_str("...");
    }
    compact
}

fn db_engine_trace(stage: &str, started: std::time::Instant) {
    if std::env::var_os("ASP_SOURCE_INDEX_TRACE").is_some() {
        eprintln!(
            "[db-engine-trace] stage={} elapsedMs={}",
            stage,
            started.elapsed().as_millis()
        );
    }
}

fn active_client_db_backend() -> ClientDbBackend {
    ClientDbBackend::Turso
}

fn turso_client_db_report(db_path: &Path) -> ClientDbReport {
    let status = if db_path.exists() {
        ClientDbStatus::Present
    } else {
        ClientDbStatus::Missing
    };
    let mut reason = None;
    let counts = if db_path.exists() {
        match turso_client_db_counts(db_path) {
            Ok(counts) => counts,
            Err(error) => {
                reason = Some(error);
                TursoClientDbCounts::default()
            }
        }
    } else {
        TursoClientDbCounts::default()
    };
    ClientDbReport {
        db_path: db_path.to_path_buf(),
        status,
        generation_count: counts.cache_generations,
        syntax_row_generation_count: counts.syntax_replays,
        syntax_row_match_count: 0,
        syntax_row_capture_count: 0,
        structural_index_generation_count: counts.structural_index_generations,
        structural_index_owner_count: counts.structural_index_owners,
        structural_index_symbol_count: counts.structural_index_symbols,
        structural_index_dependency_usage_count: counts.structural_index_dependency_usages,
        source_index_generation_count: counts.source_index_generations,
        source_index_owner_count: counts.source_index_owners,
        source_index_selector_count: counts.source_index_selectors,
        artifact_event_count: counts.artifact_events,
        raw_source_stored: false,
        runtime_pragmas: None,
        reason,
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct TursoClientDbCounts {
    cache_generations: u32,
    syntax_replays: u32,
    structural_index_generations: u32,
    structural_index_owners: u32,
    structural_index_symbols: u32,
    structural_index_dependency_usages: u32,
    source_index_generations: u32,
    source_index_owners: u32,
    source_index_selectors: u32,
    artifact_events: u32,
}

fn turso_client_db_counts(db_path: &Path) -> Result<TursoClientDbCounts, String> {
    let db_path = db_path.to_path_buf();
    block_on_db_engine_async(async move {
        let connection = connect_turso_client_db(&db_path).await?;
        Ok(TursoClientDbCounts {
            cache_generations: count_turso_rows_or_zero(&connection, "asp_cache_generation").await,
            syntax_replays: count_turso_rows_or_zero(&connection, "asp_syntax_query_replay").await,
            structural_index_generations: count_turso_structural_generations_or_zero(&connection)
                .await,
            structural_index_owners: count_turso_graph_kind_or_zero(
                &connection,
                "structural-owner",
            )
            .await,
            structural_index_symbols: count_turso_graph_kind_or_zero(&connection, "symbol").await,
            structural_index_dependency_usages: count_turso_graph_kind_or_zero(
                &connection,
                "dependency-usage",
            )
            .await,
            source_index_generations: count_turso_rows(&connection, "asp_source_index_generation")
                .await?,
            source_index_owners: count_turso_rows(&connection, "asp_source_index_owner").await?,
            source_index_selectors: count_turso_rows(&connection, "asp_source_index_selector")
                .await?,
            artifact_events: count_turso_rows_or_zero(&connection, "asp_artifact_event").await,
        })
    })
}

async fn count_turso_rows_or_zero(connection: &turso::Connection, table: &str) -> u32 {
    count_turso_rows(connection, table).await.unwrap_or(0)
}

async fn count_turso_rows(connection: &turso::Connection, table: &str) -> Result<u32, String> {
    let sql = format!("SELECT COUNT(*) FROM {table}");
    count_turso_query(connection, &sql, ())
        .await
        .or_else(|error| {
            if error.contains("no such table") {
                Ok(0)
            } else {
                Err(error)
            }
        })
}

async fn count_turso_graph_kind_or_zero(connection: &turso::Connection, kind: &str) -> u32 {
    count_turso_graph_kind(connection, kind).await.unwrap_or(0)
}

async fn count_turso_graph_kind(connection: &turso::Connection, kind: &str) -> Result<u32, String> {
    count_turso_query(
        connection,
        "SELECT COUNT(*) FROM asp_graph_entity WHERE kind = ?1",
        [kind],
    )
    .await
    .or_else(|error| {
        if error.contains("no such table") {
            Ok(0)
        } else {
            Err(error)
        }
    })
}

async fn count_turso_structural_generations_or_zero(connection: &turso::Connection) -> u32 {
    count_turso_structural_generations(connection)
        .await
        .unwrap_or(0)
}

async fn count_turso_structural_generations(connection: &turso::Connection) -> Result<u32, String> {
    let structural_entities = count_turso_query(
        connection,
        "SELECT COUNT(*) FROM asp_graph_entity WHERE kind IN ('structural-owner', 'symbol', 'dependency-usage')",
        (),
    )
    .await
    .or_else(|error| {
        if error.contains("no such table") {
            Ok(0)
        } else {
            Err(error)
        }
    })?;
    Ok((structural_entities > 0) as u32)
}

async fn count_turso_query<P>(
    connection: &turso::Connection,
    sql: &str,
    params: P,
) -> Result<u32, String>
where
    P: turso::params::IntoParams,
{
    let mut rows = connection
        .query(sql, params)
        .await
        .map_err(|error| format!("failed to count Turso DB rows for `{sql}`: {error}"))?;
    let Some(row) = rows
        .next()
        .await
        .map_err(|error| format!("failed to read Turso DB row count for `{sql}`: {error}"))?
    else {
        return Ok(0);
    };
    let count = row
        .get::<i64>(0)
        .map_err(|error| format!("failed to decode Turso DB row count for `{sql}`: {error}"))?;
    Ok(count.max(0).min(i64::from(u32::MAX)) as u32)
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

fn source_index_busy_lookup_result(db_path: PathBuf) -> ClientDbSourceIndexLookupResult {
    source_index_lookup_result(db_path, ClientDbSourceIndexLookupState::Busy, Vec::new())
}

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

async fn persist_structural_index_search_documents_at_path(
    db_path: &Path,
    generation_id: &str,
    graph: &crate::ClientDbEvidenceGraph,
) -> Result<usize, String> {
    let mut documents = Vec::new();
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
        documents.push(document);
    }
    upsert_turso_search_documents(db_path, &documents).await
}

async fn persist_structural_index_read_model_at_path(
    db_path: &Path,
    import: &ClientDbStructuralIndexImport,
) -> Result<ClientDbEngineStructuralIndexReadModelReport, String> {
    let trace_started = std::time::Instant::now();
    bootstrap_turso_client_db(db_path).await?;
    db_engine_trace("structural-index-bootstrap", trace_started);
    let graph = structural_index_evidence_graph(import);
    db_engine_trace("structural-index-graph-built", trace_started);
    let graph_report = persist_turso_evidence_graph(db_path, &graph).await?;
    db_engine_trace("structural-index-graph-persisted", trace_started);
    let search_document_count = persist_structural_index_search_documents_at_path(
        db_path,
        import.generation_id.as_str(),
        &graph,
    )
    .await?;
    db_engine_trace("structural-index-search-documents-persisted", trace_started);
    Ok(structural_index_read_model_report(
        graph_report,
        search_document_count,
    ))
}

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
    let connection = match connect_turso_client_db(&db_path).await {
        Ok(connection) => connection,
        Err(error) if is_turso_lock_error(&error) => {
            return Ok(source_index_busy_lookup_result(db_path));
        }
        Err(error) => return Err(error),
    };
    let tables_exist = match turso_source_index_lookup_tables_exist(&connection).await {
        Ok(tables_exist) => tables_exist,
        Err(error) if is_turso_lock_error(&error) => {
            return Ok(source_index_busy_lookup_result(db_path));
        }
        Err(error) => return Err(error),
    };
    if !tables_exist {
        return Ok(source_index_lookup_result(
            db_path,
            ClientDbSourceIndexLookupState::EmptyIndex,
            Vec::new(),
        ));
    }
    let candidates = match query_turso_source_index_candidates_with_connection(
        &connection,
        query,
        language_id,
        limit,
        &terms,
    )
    .await
    {
        Ok(candidates) => candidates,
        Err(error) if is_turso_lock_error(&error) => {
            return Ok(source_index_busy_lookup_result(db_path));
        }
        Err(error) => return Err(error),
    };
    let owner_rows_exist = match turso_source_index_owner_rows_exist(&connection).await {
        Ok(owner_rows_exist) => owner_rows_exist,
        Err(error) if is_turso_lock_error(&error) => {
            return Ok(source_index_busy_lookup_result(db_path));
        }
        Err(error) => return Err(error),
    };
    if candidates.is_empty() && !owner_rows_exist {
        return Ok(source_index_lookup_result(
            db_path,
            ClientDbSourceIndexLookupState::EmptyIndex,
            Vec::new(),
        ));
    }
    let state = if candidates.is_empty() {
        ClientDbSourceIndexLookupState::Miss
    } else {
        ClientDbSourceIndexLookupState::Hit
    };
    Ok(source_index_lookup_result(db_path, state, candidates))
}

async fn turso_source_index_lookup_tables_exist(
    connection: &turso::Connection,
) -> Result<bool, String> {
    for table_name in [
        "asp_source_index_generation",
        "asp_source_index_owner",
        "asp_source_index_selector",
    ] {
        if !turso_table_exists(connection, table_name).await? {
            return Ok(false);
        }
    }
    Ok(true)
}

async fn turso_source_index_owner_rows_exist(
    connection: &turso::Connection,
) -> Result<bool, String> {
    let mut rows = connection
        .query("SELECT owner_path FROM asp_source_index_owner LIMIT 1", ())
        .await
        .map_err(|error| format!("failed to inspect Turso source-index owner rows: {error}"))?;
    Ok(rows
        .next()
        .await
        .map_err(|error| format!("failed to read Turso source-index owner rows: {error}"))?
        .is_some())
}

async fn query_turso_source_index_candidates_with_connection(
    connection: &turso::Connection,
    query: &str,
    language_id: Option<&LanguageId>,
    limit: u32,
    terms: &[String],
) -> Result<Vec<ClientDbSourceIndexCandidate>, String> {
    if limit == 0 || query.trim().is_empty() {
        return Ok(Vec::new());
    }
    let mut generation_rows = connection
        .query(
            "SELECT generation_id
             FROM asp_source_index_generation
             ORDER BY updated_at_ms DESC
             LIMIT 1",
            (),
        )
        .await
        .map_err(|error| {
            format!("failed to query Turso source-index latest generation: {error}")
        })?;
    let Some(generation_row) = generation_rows
        .next()
        .await
        .map_err(|error| format!("failed to read Turso source-index latest generation: {error}"))?
    else {
        return Ok(Vec::new());
    };
    let generation_id = generation_row
        .get::<String>(0)
        .map_err(|error| format!("failed to read Turso source-index generation id: {error}"))?;
    drop(generation_rows);

    let mut selector_rows = connection
        .query(
            "SELECT owner_path, selector_id, COALESCE(symbol, ''), kind, COALESCE(source, ''), query_keys_json
             FROM asp_source_index_selector
             WHERE generation_id = ?1
             ORDER BY owner_path, selector_id",
            (generation_id.as_str(),),
        )
        .await
        .map_err(|error| format!("failed to query Turso source-index selectors: {error}"))?;
    let mut selector_haystacks = std::collections::BTreeMap::<String, String>::new();
    while let Some(row) = selector_rows
        .next()
        .await
        .map_err(|error| format!("failed to read Turso source-index selector row: {error}"))?
    {
        let owner_path = row.get::<String>(0).map_err(|error| {
            format!("failed to read Turso source-index selector owner path: {error}")
        })?;
        let selector_id = row
            .get::<String>(1)
            .map_err(|error| format!("failed to read Turso source-index selector id: {error}"))?;
        let symbol = row.get::<String>(2).map_err(|error| {
            format!("failed to read Turso source-index selector symbol: {error}")
        })?;
        let kind = row
            .get::<String>(3)
            .map_err(|error| format!("failed to read Turso source-index selector kind: {error}"))?;
        let source = row.get::<String>(4).map_err(|error| {
            format!("failed to read Turso source-index selector source: {error}")
        })?;
        let query_keys_json = row.get::<String>(5).map_err(|error| {
            format!("failed to read Turso source-index selector query keys: {error}")
        })?;
        let haystack = selector_haystacks.entry(owner_path).or_default();
        haystack.push(' ');
        haystack.push_str(&selector_id);
        haystack.push(' ');
        haystack.push_str(&symbol);
        haystack.push(' ');
        haystack.push_str(&kind);
        haystack.push(' ');
        haystack.push_str(&source);
        haystack.push(' ');
        haystack.push_str(&query_keys_json);
    }
    drop(selector_rows);

    let mut rows = connection
        .query(
            "SELECT owner_path, language_id, provider_id, source_kind, line_count, query_keys_json
             FROM asp_source_index_owner
             WHERE generation_id = ?1
             ORDER BY owner_path",
            (generation_id.as_str(),),
        )
        .await
        .map_err(|error| format!("failed to query Turso source-index owners: {error}"))?;

    let mut candidates = Vec::<(usize, ClientDbSourceIndexCandidate)>::new();
    while let Some(row) = rows
        .next()
        .await
        .map_err(|error| format!("failed to read Turso source-index owner row: {error}"))?
    {
        let path = row
            .get::<String>(0)
            .map_err(|error| format!("failed to read Turso source-index owner path: {error}"))?;
        let row_language_id = row.get::<Option<String>>(1).map_err(|error| {
            format!("failed to read Turso source-index owner language id: {error}")
        })?;
        if let Some(language_id) = language_id
            && row_language_id.as_deref() != Some(language_id.as_str())
        {
            continue;
        }
        let provider_id = row.get::<Option<String>>(2).map_err(|error| {
            format!("failed to read Turso source-index owner provider id: {error}")
        })?;
        let source_kind = row.get::<String>(3).map_err(|error| {
            format!("failed to read Turso source-index owner source kind: {error}")
        })?;
        let line_count = row
            .get::<Option<i64>>(4)
            .map_err(|error| format!("failed to read Turso source-index line count: {error}"))?
            .and_then(|value| u32::try_from(value).ok());
        let query_keys_json = row
            .get::<String>(5)
            .map_err(|error| format!("failed to read Turso source-index query keys: {error}"))?;
        let query_keys = serde_json::from_str::<Vec<String>>(&query_keys_json)
            .map_err(|error| format!("failed to decode Turso source-index query keys: {error}"))?;
        let selector_haystack = selector_haystacks
            .get(&path)
            .map(String::as_str)
            .unwrap_or_default();
        let match_score = source_index_structured_candidate_score(
            &path,
            row_language_id.as_deref(),
            provider_id.as_deref(),
            &source_kind,
            &query_keys,
            &selector_haystack,
            terms,
        );
        if match_score == 0 {
            continue;
        }
        let language_id = row_language_id.map(LanguageId::from);
        let provider_id = provider_id.map(ProviderId::from);
        if candidates.iter().any(|(_, candidate)| {
            candidate.path == path
                && candidate.language_id == language_id
                && candidate.provider_id == provider_id
        }) {
            continue;
        }
        candidates.push((
            match_score,
            ClientDbSourceIndexCandidate {
                path,
                language_id,
                provider_id,
                source_kind: ClientDbSourceIndexSourceKind::Other("turso-source-index".to_string()),
                line_count,
                query_keys,
            },
        ));
    }
    candidates.sort_by(|(left_score, left), (right_score, right)| {
        right_score
            .cmp(left_score)
            .then_with(|| left.path.cmp(&right.path))
    });
    Ok(candidates
        .into_iter()
        .map(|(_, candidate)| candidate)
        .take(limit as usize)
        .collect())
}

fn source_index_structured_candidate_score(
    path: &str,
    language_id: Option<&str>,
    provider_id: Option<&str>,
    source_kind: &str,
    query_keys: &[String],
    selector_haystack: &str,
    terms: &[String],
) -> usize {
    if terms.is_empty() {
        return 1;
    }
    let mut haystack = String::new();
    haystack.push_str(path);
    haystack.push(' ');
    if let Some(language_id) = language_id {
        haystack.push_str(language_id);
        haystack.push(' ');
    }
    if let Some(provider_id) = provider_id {
        haystack.push_str(provider_id);
        haystack.push(' ');
    }
    haystack.push_str(source_kind);
    haystack.push(' ');
    for query_key in query_keys {
        haystack.push_str(query_key);
        haystack.push(' ');
    }
    haystack.push_str(selector_haystack);
    let haystack = haystack.to_lowercase();
    terms.iter().filter(|term| haystack.contains(*term)).count()
}

fn source_index_read_model_terms(query: &str) -> Vec<String> {
    query
        .split(|character: char| {
            !character.is_alphanumeric() && character != '_' && character != '-'
        })
        .filter(|term| !term.is_empty())
        .map(|term| term.to_ascii_lowercase())
        .collect()
}
