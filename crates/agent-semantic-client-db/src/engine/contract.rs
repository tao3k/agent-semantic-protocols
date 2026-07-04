//! Shared DB Engine backend contract types.

use agent_semantic_client_core::state_core::TURSO_BACKEND;
use serde::Serialize;

/// Current durable client DB backend selected by the ASP DB Engine.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ClientDbBackend {
    /// Turso/libSQL backend for the ASP DB Engine.
    Turso,
}

impl ClientDbBackend {
    /// Stable manifest token for this backend.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Turso => TURSO_BACKEND,
        }
    }
}

/// Durability class for the selected DB Engine backend.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ClientDbEngineDurability {
    /// Local Turso/libSQL durable file owned by the DB Engine.
    TursoLocalFile,
}

impl ClientDbEngineDurability {
    /// Stable receipt token for this durability class.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
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
    pub fts_index_method: bool,
    pub vector: bool,
    pub overlay_search: bool,
    pub sync: bool,
    pub encryption: bool,
    pub multi_process_wal: bool,
    pub serialized_writer_slot: bool,
    pub busy_timeout_ms: u64,
    pub open_lock_retry_attempts: usize,
    pub open_lock_retry_base_ms: u64,
    pub open_lock_retry_max_ms: u64,
    pub statement_lock_retry_attempts: usize,
    pub operation_lock: bool,
    pub operation_lock_retry_attempts: usize,
    pub operation_lock_retry_ms: u64,
    pub mvcc: bool,
    pub begin_concurrent: bool,
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

    /// Inspect the backend without creating a DB file.
    fn inspect(&self, db_path: &std::path::Path) -> Self::Report;
}
