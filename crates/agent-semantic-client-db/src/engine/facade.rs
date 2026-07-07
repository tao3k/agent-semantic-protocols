//! ASP-owned client DB engine facade.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use agent_semantic_client_core::{
    AGENT_SEMANTIC_CLIENT_CACHE_MANIFEST_FILE, ClientCacheGeneration, ClientCacheManifest,
    ClientDbJournalMode, ClientDbStatus, LanguageId, ProviderId,
    state_core::{ResolvedState, STATE_LAYOUT_VERSION, STATE_MANIFEST_FILE, TURSO_BACKEND},
};
use serde::Serialize;
use serde_json::json;

use crate::source_index::{ClientDbSourceIndexRefreshReport, ClientDbSourceIndexRefreshRequest};
use crate::structural_index::parse_structural_index_packet_import;
use crate::types::{
    ClientDbArtifactEdge, ClientDbArtifactEvent, ClientDbArtifactGraphCompactRender,
    ClientDbArtifactRepairChainFrame, ClientDbArtifactRoot, ClientDbProofReceipt,
    ClientDbProviderCommandSelection, ClientDbReport, ClientDbRuntimePragmas,
    ClientDbSyntaxQueryLookup, ClientDbSyntaxQueryReplay,
};

use super::contract::{ClientDbBackend, ClientDbEngineBackend, ClientDbEngineFeatures};
use super::source_index_facade::persist_structural_index_read_model_at_path;
use super::turso::connect_turso_client_db;
use super::turso::{TursoClientDbEngineBackend, TursoClientDbEngineReport};
use super::turso_artifact::{lookup_turso_artifact_events, upsert_turso_artifact_events};
use super::turso_artifact_graph::{
    lookup_turso_artifact_edges, lookup_turso_proof_receipts, lookup_turso_repair_chain_frames,
    upsert_turso_artifact_edges, upsert_turso_artifact_roots, upsert_turso_proof_receipts,
    upsert_turso_repair_chain_frames,
};
use super::turso_bootstrap::bootstrap_turso_client_db;
use super::turso_cache::{
    invalidate_turso_cache_generations_for_project, prune_turso_cache_generations_to_manifest,
    upsert_turso_cache_generations,
};
use super::turso_lock_policy::TURSO_CLIENT_DB_BUSY_TIMEOUT_MS;
use super::turso_provider_command::{
    lookup_turso_provider_command_selections, replace_turso_provider_command_selections,
};
use super::turso_route_receipt::{
    TursoClientDbRouteReceipt, list_turso_route_receipts, upsert_turso_route_receipt,
};
use super::turso_search::{TursoClientDbSearchHit, search_turso_documents};
use super::turso_source_index::refresh_turso_source_index_import;
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

impl ClientDbEngineReport {
    #[must_use]
    pub fn backend(&self) -> &'static str {
        self.backend
    }

    #[must_use]
    pub fn layout_version(&self) -> &'static str {
        self.layout_version
    }

    #[must_use]
    pub fn db_file_name(&self) -> &'static str {
        self.db_file_name
    }

    #[must_use]
    pub fn schema_version(&self) -> i64 {
        self.schema_version
    }

    #[must_use]
    pub fn durability(&self) -> &'static str {
        self.durability
    }

    #[must_use]
    pub fn features(&self) -> &ClientDbEngineFeatures {
        &self.features
    }

    #[must_use]
    pub fn client_dir(&self) -> &Path {
        &self.client_dir
    }

    #[must_use]
    pub fn db_path(&self) -> &Path {
        &self.db_path
    }

    #[must_use]
    pub fn manifest_path(&self) -> &Path {
        &self.manifest_path
    }

    #[must_use]
    pub fn artifact_path(&self) -> &Path {
        &self.artifact_path
    }

    #[must_use]
    pub fn repo_id(&self) -> &str {
        &self.repo_id
    }

    #[must_use]
    pub fn workspace_id(&self) -> &str {
        &self.workspace_id
    }

    #[must_use]
    pub fn scope_id(&self) -> &str {
        &self.scope_id
    }
}

/// DB Engine read session over the active Turso adapter.
pub struct ClientDbEngineReadSession {
    pub(super) turso_db_path: PathBuf,
}

/// DB Engine write session over the active Turso adapter.
pub struct ClientDbEngineWriteSession {
    pub(super) turso_db_path: PathBuf,
}

pub(super) fn block_on_db_engine_async<T, F>(future: F) -> Result<T, String>
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
        if !turso_db_path.exists() {
            let bootstrap_path = turso_db_path.clone();
            block_on_db_engine_async(async move {
                bootstrap_turso_client_db(&bootstrap_path).await.map(|_| ())
            })?;
        }
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

    /// Search overlay/stable documents through the active DB Engine backend.
    pub fn search_documents_from_client_dir(
        client_dir: impl AsRef<Path>,
        query: &str,
        limit: u32,
    ) -> Result<Vec<TursoClientDbSearchHit>, String> {
        let db_path = Self::turso_path_for_client_dir(client_dir.as_ref());
        let query = query.to_string();
        block_on_db_engine_async(
            async move { search_turso_documents(&db_path, &query, limit).await },
        )
    }

    /// Search overlay/stable documents using this resolved DB Engine.
    pub fn search_documents_blocking(
        &self,
        query: &str,
        limit: u32,
    ) -> Result<Vec<TursoClientDbSearchHit>, String> {
        Self::search_documents_from_client_dir(&self.client_dir, query, limit)
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

    /// List recent route receipts through the active DB Engine backend.
    pub async fn list_route_receipts(
        &self,
        session_id: Option<&str>,
        limit: u32,
    ) -> Result<Vec<TursoClientDbRouteReceipt>, String> {
        self.bootstrap_active_turso().await?;
        list_turso_route_receipts(&self.db_path, session_id, limit).await
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

fn active_client_db_backend() -> ClientDbBackend {
    ClientDbBackend::Turso
}

pub(super) fn turso_client_db_report(db_path: &Path) -> ClientDbReport {
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
    let runtime_pragmas_available = status == ClientDbStatus::Present;
    ClientDbReport {
        db_path: db_path.to_path_buf(),
        status,
        generation_count: counts.cache_generations,
        syntax_row_generation_count: counts.syntax_replays,
        syntax_row_match_count: counts.syntax_row_matches,
        syntax_row_capture_count: counts.syntax_row_captures,
        structural_index_generation_count: counts.structural_index_generations,
        structural_index_owner_count: counts.structural_index_owners,
        structural_index_symbol_count: counts.structural_index_symbols,
        structural_index_dependency_usage_count: counts.structural_index_dependency_usages,
        source_index_generation_count: counts.source_index_generations,
        source_index_owner_count: counts.source_index_owners,
        source_index_selector_count: counts.source_index_selectors,
        artifact_event_count: counts.artifact_events,
        raw_source_stored: false,
        runtime_pragmas: runtime_pragmas_available.then(|| ClientDbRuntimePragmas {
            journal_mode: ClientDbJournalMode::from("wal"),
            synchronous: 1,
            busy_timeout_ms: TURSO_CLIENT_DB_BUSY_TIMEOUT_MS as i64,
            foreign_keys: true,
        }),
        reason,
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct TursoClientDbCounts {
    cache_generations: u32,
    syntax_replays: u32,
    syntax_row_matches: u32,
    syntax_row_captures: u32,
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
        let syntax_row_counts = count_turso_syntax_replay_rows_or_zero(&connection).await;
        Ok(TursoClientDbCounts {
            cache_generations: count_turso_rows_or_zero(&connection, "asp_cache_generation").await,
            syntax_replays: count_turso_rows_or_zero(&connection, "asp_syntax_query_replay").await,
            syntax_row_matches: syntax_row_counts.matches,
            syntax_row_captures: syntax_row_counts.captures,
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

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct TursoSyntaxReplayRowCounts {
    matches: u32,
    captures: u32,
}

async fn count_turso_syntax_replay_rows_or_zero(
    connection: &turso::Connection,
) -> TursoSyntaxReplayRowCounts {
    count_turso_syntax_replay_rows(connection)
        .await
        .unwrap_or_default()
}

async fn count_turso_syntax_replay_rows(
    connection: &turso::Connection,
) -> Result<TursoSyntaxReplayRowCounts, String> {
    let mut rows = connection
        .query("SELECT replay_json FROM asp_syntax_query_replay", ())
        .await
        .map_err(|error| format!("failed to query Turso syntax replay rows: {error}"))?;
    let mut counts = TursoSyntaxReplayRowCounts::default();
    while let Some(row) = rows
        .next()
        .await
        .map_err(|error| format!("failed to read Turso syntax replay row: {error}"))?
    {
        let replay_json = row
            .get::<String>(0)
            .map_err(|error| format!("failed to read Turso syntax replay JSON: {error}"))?;
        let replay = serde_json::from_str::<ClientDbSyntaxQueryReplay>(&replay_json)
            .map_err(|error| format!("failed to decode Turso syntax replay JSON: {error}"))?;
        let matches = replay
            .rows
            .iter()
            .map(|row| row.match_locator.as_str())
            .collect::<BTreeSet<_>>()
            .len();
        counts.matches = counts
            .matches
            .saturating_add(matches.min(u32::MAX as usize) as u32);
        counts.captures = counts
            .captures
            .saturating_add(replay.rows.len().min(u32::MAX as usize) as u32);
    }
    Ok(counts)
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
