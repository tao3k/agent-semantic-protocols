//! Shared DB Engine backend contract types.

use std::path::Path;

use agent_semantic_client_core::state_core::{SQLITE_V1_BACKEND, TURSO_BACKEND};
use serde::Serialize;

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
