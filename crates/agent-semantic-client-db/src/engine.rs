//! ASP-owned client DB engine facade.

use std::path::{Path, PathBuf};

use agent_semantic_client_core::state_core::{
    CLIENT_DB_FILE, ResolvedState, SQLITE_V1_BACKEND, STATE_LAYOUT_VERSION, STATE_MANIFEST_FILE,
    TURSO_BACKEND,
};

use crate::db::{ClientDb, ClientDbReport};

/// Current durable client DB backend selected by the ASP DB Engine.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ClientDbBackend {
    /// Existing local SQLite schema and rusqlite adapter.
    SqliteV1,
}

impl ClientDbBackend {
    /// Stable manifest token for this backend.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::SqliteV1 => SQLITE_V1_BACKEND,
        }
    }
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
        Self {
            backend: ClientDbBackend::SqliteV1,
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

    /// Return the SQLite v1 DB path below an already resolved client directory.
    #[must_use]
    pub fn sqlite_path_for_client_dir(client_dir: impl AsRef<Path>) -> PathBuf {
        client_dir.as_ref().join(CLIENT_DB_FILE)
    }

    /// Return the DB manifest path below an already resolved client directory.
    #[must_use]
    pub fn manifest_path_for_client_dir(client_dir: impl AsRef<Path>) -> PathBuf {
        client_dir.as_ref().join(STATE_MANIFEST_FILE)
    }

    /// Open the current SQLite v1 backend and run idempotent schema migration.
    pub fn open_sqlite_or_create(&self) -> Result<ClientDb, String> {
        ClientDb::open_or_create(&self.db_path)
    }

    /// Open the current SQLite v1 backend read-only when the file exists.
    pub fn open_sqlite_read_only_existing(&self) -> Result<Option<ClientDb>, String> {
        ClientDb::open_read_only_existing(&self.db_path)
    }

    /// Inspect the current SQLite v1 backend without creating a DB file.
    #[must_use]
    pub fn inspect_sqlite(&self) -> ClientDbReport {
        ClientDb::inspect(&self.db_path)
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
}
