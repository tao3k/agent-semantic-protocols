//! ASP-owned client DB engine facade.

use std::fs;
use std::path::{Path, PathBuf};

use agent_semantic_client_core::{
    AGENT_SEMANTIC_CLIENT_CACHE_MANIFEST_FILE, ClientCacheGeneration, ClientCacheManifest,
    LanguageId, ProviderId,
    state_core::{ResolvedState, STATE_LAYOUT_VERSION, STATE_MANIFEST_FILE, TURSO_BACKEND},
};
use serde::Serialize;
use serde_json::json;

use crate::source_index::{ClientDbSourceIndexRefreshReport, ClientDbSourceIndexRefreshRequest};
use crate::structural_index::parse_structural_index_packet_import;
use crate::types::{
    ClientDbArtifactEdge, ClientDbArtifactEvent, ClientDbArtifactGraphCompactRender,
    ClientDbArtifactRepairChainFrame, ClientDbArtifactRoot, ClientDbProofReceipt,
    ClientDbProviderCommandSelection, ClientDbReport, ClientDbSyntaxQueryLookup,
    ClientDbSyntaxQueryReplay,
};

use super::contract::{ClientDbBackend, ClientDbEngineBackend, ClientDbEngineFeatures};
use super::source_index_facade::persist_structural_index_read_model_at_path;
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
use super::turso_provider_command::{
    lookup_turso_provider_command_selections, replace_turso_provider_command_selections,
};
use super::turso_source_index::refresh_turso_source_index_import;
use super::turso_syntax::{
    flush_turso_syntax_query_replay, lookup_turso_syntax_query_replay,
    upsert_turso_syntax_query_replay,
};

macro_rules! client_db_engine_id {
    ($(#[$meta:meta])* $name:ident) => {
        $(#[$meta])*
        #[derive(Clone, Debug, Eq, PartialEq, Serialize)]
        pub struct $name(String);

        impl $name {
            #[must_use]
            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl From<String> for $name {
            fn from(value: String) -> Self {
                Self(value)
            }
        }

        impl From<&str> for $name {
            fn from(value: &str) -> Self {
                Self(value.to_string())
            }
        }
    };
}

client_db_engine_id!(
    /// State Core repository id owned by the DB engine.
    ClientDbRepoId
);
client_db_engine_id!(
    /// State Core workspace id owned by the DB engine.
    ClientDbWorkspaceId
);
client_db_engine_id!(
    /// State Core scope id owned by the DB engine.
    ClientDbScopeId
);

/// Resolved DB Engine paths and backend selection for one State Core workspace.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClientDbEngine {
    backend: ClientDbBackend,
    layout_version: &'static str,
    client_dir: PathBuf,
    db_path: PathBuf,
    manifest_path: PathBuf,
    artifact_path: PathBuf,
    repo_id: ClientDbRepoId,
    workspace_id: ClientDbWorkspaceId,
    scope_id: ClientDbScopeId,
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
    pub repo_id: ClientDbRepoId,
    pub workspace_id: ClientDbWorkspaceId,
    pub scope_id: ClientDbScopeId,
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
        self.repo_id.as_str()
    }

    #[must_use]
    pub fn workspace_id(&self) -> &str {
        self.workspace_id.as_str()
    }

    #[must_use]
    pub fn scope_id(&self) -> &str {
        self.scope_id.as_str()
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
    // Turso databases and connection lanes are process-scoped. Keep their async
    // driver alive for the same lifetime instead of pooling them across runtimes
    // that are destroyed after each synchronous facade call.
    static DB_ENGINE_RUNTIME: std::sync::OnceLock<Result<tokio::runtime::Runtime, String>> =
        std::sync::OnceLock::new();

    std::thread::spawn(move || {
        let runtime = DB_ENGINE_RUNTIME.get_or_init(|| {
            tokio::runtime::Builder::new_multi_thread()
                .worker_threads(2)
                .thread_name("asp-client-db")
                .enable_all()
                .build()
                .map_err(|error| format!("failed to build DB Engine async runtime: {error}"))
        });
        match runtime {
            Ok(runtime) => runtime.block_on(future),
            Err(error) => Err(error.clone()),
        }
    })
    .join()
    .map_err(|_| "DB Engine async runtime thread panicked".to_string())?
}

/// DB Engine receipt for projecting a source-index import into Turso read models.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClientDbEngineSourceIndexReadModelReport {
    pub node_locator_count: usize,
    pub search_document_count: usize,
}

/// DB Engine receipt for projecting a structural-index import into Turso read models.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClientDbEngineStructuralIndexReadModelReport {
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
            repo_id: ClientDbRepoId::from(state.repo.repo_id.to_string()),
            workspace_id: ClientDbWorkspaceId::from(state.workspace.workspace_id.to_string()),
            scope_id: ClientDbScopeId::from(state.scope_id.to_string()),
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
        source_snapshot: &agent_semantic_content_identity::SourceSnapshotEvidence,
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
        let source_snapshot = source_snapshot.clone();
        block_on_db_engine_async(async move {
            persist_structural_index_read_model_at_path(&db_path, &import, &source_snapshot)
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
            invalidate_turso_cache_generations_for_project(&db_path, &project_root).await
        })
    }

    /// Inspect the active DB Engine adapter for an already resolved client directory.
    #[must_use]
    pub fn inspect_client_dir(client_dir: impl AsRef<Path>) -> ClientDbReport {
        crate::engine::facade_turso_report::turso_client_db_report(
            &Self::turso_path_for_client_dir(client_dir),
        )
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
        crate::engine::facade_turso_report::turso_client_db_report(&self.db_path)
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
        self.repo_id.as_str()
    }

    /// Stable State Core workspace identity.
    #[must_use]
    pub fn workspace_id(&self) -> &str {
        self.workspace_id.as_str()
    }

    /// Stable State Core scope identity.
    #[must_use]
    pub fn scope_id(&self) -> &str {
        self.scope_id.as_str()
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
