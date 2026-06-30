//! ASP-owned client DB engine facade.

use std::path::{Path, PathBuf};

use agent_semantic_client_core::state_core::{
    CLIENT_DB_FILE, ResolvedState, SQLITE_V1_BACKEND, STATE_LAYOUT_VERSION, STATE_MANIFEST_FILE,
    TURSO_BACKEND,
};
use serde::Serialize;

use crate::db::{ClientDb, ClientDbReport};

use super::sqlite::SqliteClientDbEngineBackend;
use super::turso::{TursoClientDbEngineBackend, TursoClientDbEngineReport};

/// Current durable client DB backend selected by the ASP DB Engine.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ClientDbBackend {
    /// Existing local SQLite schema and rusqlite adapter.
    SqliteV1,
    /// Target Turso/libSQL backend for the new DB Engine.
    Turso,
}

impl ClientDbBackend {
    /// Stable manifest token for this backend.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::SqliteV1 => SQLITE_V1_BACKEND,
            Self::Turso => TURSO_BACKEND,
        }
    }
}

/// Durability class for the selected DB Engine backend.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ClientDbEngineDurability {
    /// Transitional SQLite file managed by the phase-1 `rusqlite` adapter.
    SqliteFile,
    /// Local Turso/libSQL durable file owned by the DB Engine.
    TursoLocalFile,
}

impl ClientDbEngineDurability {
    /// Stable receipt token for this durability class.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::SqliteFile => "sqlite-file",
            Self::TursoLocalFile => "turso-local-file",
        }
    }
}

/// Capability flags reported by a DB Engine backend.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClientDbEngineFeatures {
    pub async_io: bool,
    pub concurrent_writes: bool,
    pub fts: bool,
    pub vector: bool,
    pub overlay_search: bool,
    pub sync: bool,
    pub encryption: bool,
}

/// Backend adapter boundary used by the ASP DB Engine facade.
pub(super) trait ClientDbEngineBackend {
    /// Open connection type for this backend.
    type Connection;

    /// Diagnostic report emitted by this backend.
    type Report;

    /// Stable backend token recorded in manifests and receipts.
    fn backend(&self) -> ClientDbBackend;

    /// DB file name used below the State Core client directory.
    fn db_file_name(&self) -> &'static str;

    /// Released schema version for this backend.
    fn schema_version(&self) -> i64;

    /// Durability class used by analyzer receipts and migration gates.
    fn durability(&self) -> ClientDbEngineDurability;

    /// Backend capability flags used by migration and benchmark gates.
    fn features(&self) -> ClientDbEngineFeatures;

    /// Open the backend and create or migrate its schema when needed.
    fn open_or_create(&self, db_path: &Path) -> Result<Self::Connection, String>;

    /// Open the backend read-only when its DB file already exists.
    fn open_read_only_existing(&self, db_path: &Path) -> Result<Option<Self::Connection>, String>;

    /// Inspect the backend without creating a DB file.
    fn inspect(&self, db_path: &Path) -> Self::Report;
}

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

impl ClientDbEngine {
    /// Resolve the DB Engine from State Core and create the minimal layout.
    pub fn resolve(project_root: impl AsRef<Path>) -> Result<Self, String> {
        let state = ResolvedState::resolve(project_root)?;
        state.ensure_minimal_layout()?;
        Ok(Self::from_resolved_state(&state))
    }

    /// Build an engine descriptor from an already resolved State Core value.
    #[must_use]
    pub fn from_resolved_state(state: &ResolvedState) -> Self {
        let backend = SqliteClientDbEngineBackend.backend();
        Self {
            backend,
            future_backend: TURSO_BACKEND,
            layout_version: STATE_LAYOUT_VERSION,
            client_dir: state.paths.client_dir.clone(),
            db_path: state.paths.client_db_path.clone(),
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
        client_dir.as_ref().join(CLIENT_DB_FILE)
    }

    /// Return the SQLite v1 DB path below an already resolved client directory.
    #[must_use]
    pub fn sqlite_path_for_client_dir(client_dir: impl AsRef<Path>) -> PathBuf {
        Self::db_path_for_client_dir(client_dir)
    }

    /// Return the planned Turso DB path below an already resolved client directory.
    #[must_use]
    pub fn turso_path_for_client_dir(client_dir: impl AsRef<Path>) -> PathBuf {
        TursoClientDbEngineBackend
            .inspect(&Self::db_path_for_client_dir(client_dir))
            .db_path
    }

    /// Open the current DB Engine backend for an already resolved client directory.
    pub fn open_or_create_client_dir(client_dir: impl AsRef<Path>) -> Result<ClientDb, String> {
        SqliteClientDbEngineBackend.open_or_create(&Self::db_path_for_client_dir(client_dir))
    }

    /// Open the current DB Engine backend read-only for an already resolved client directory.
    pub fn open_read_only_existing_client_dir(
        client_dir: impl AsRef<Path>,
    ) -> Result<Option<ClientDb>, String> {
        SqliteClientDbEngineBackend
            .open_read_only_existing(&Self::db_path_for_client_dir(client_dir))
    }

    /// Inspect the current DB Engine backend for an already resolved client directory.
    #[must_use]
    pub fn inspect_client_dir(client_dir: impl AsRef<Path>) -> ClientDbReport {
        SqliteClientDbEngineBackend.inspect(&Self::db_path_for_client_dir(client_dir))
    }

    /// Inspect the planned Turso DB Engine backend for an already resolved client directory.
    #[must_use]
    pub fn inspect_turso_client_dir(client_dir: impl AsRef<Path>) -> TursoClientDbEngineReport {
        TursoClientDbEngineBackend.inspect(&Self::db_path_for_client_dir(client_dir))
    }

    /// Return the DB manifest path below an already resolved client directory.
    #[must_use]
    pub fn manifest_path_for_client_dir(client_dir: impl AsRef<Path>) -> PathBuf {
        client_dir.as_ref().join(STATE_MANIFEST_FILE)
    }

    /// Open the current DB Engine backend and run idempotent schema migration.
    pub fn open_or_create(&self) -> Result<ClientDb, String> {
        self.sqlite_backend().open_or_create(&self.db_path)
    }

    /// Open the current DB Engine backend read-only when the file exists.
    pub fn open_read_only_existing(&self) -> Result<Option<ClientDb>, String> {
        self.sqlite_backend().open_read_only_existing(&self.db_path)
    }

    /// Inspect the current DB Engine backend without creating a DB file.
    #[must_use]
    pub fn inspect_backend(&self) -> ClientDbReport {
        self.sqlite_backend().inspect(&self.db_path)
    }

    /// Inspect the current engine selection and active SQLite v1 adapter.
    #[must_use]
    pub fn inspect(&self) -> ClientDbEngineReport {
        let backend = self.sqlite_backend();
        let future_backend_report = self.turso_backend().inspect(&self.db_path);
        ClientDbEngineReport {
            backend: self.backend.as_str(),
            future_backend: self.future_backend,
            layout_version: self.layout_version,
            db_file_name: backend.db_file_name(),
            schema_version: backend.schema_version(),
            durability: backend.durability().as_str(),
            features: backend.features(),
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
}
