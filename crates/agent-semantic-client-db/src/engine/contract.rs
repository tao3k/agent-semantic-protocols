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
    /// Backend operations can use async I/O without blocking the caller runtime.
    pub(crate) async_io: bool,
    /// Backend can accept concurrent writer intent through its engine policy.
    pub(crate) concurrent_writes: bool,
    /// Backend exposes full-text search for durable indexed content.
    pub(crate) fts: bool,
    /// Backend reports which full-text index method is active.
    pub(crate) fts_index_method: bool,
    /// Backend exposes vector search primitives.
    pub(crate) vector: bool,
    /// Backend can serve non-durable dynamic overlay search.
    pub(crate) overlay_search: bool,
    /// Backend can synchronize with a remote or replica target.
    pub(crate) sync: bool,
    /// Backend can encrypt persisted data.
    pub(crate) encryption: bool,
    /// Backend read/write behavior is based on MVCC semantics.
    pub(crate) mvcc: bool,
    /// Backend supports begin-concurrent style optimistic writes.
    pub(crate) begin_concurrent: bool,
}

impl ClientDbEngineFeatures {
    #[must_use]
    pub fn async_io(&self) -> bool {
        self.async_io
    }

    #[must_use]
    pub fn concurrent_writes(&self) -> bool {
        self.concurrent_writes
    }

    #[must_use]
    pub fn fts(&self) -> bool {
        self.fts
    }

    #[must_use]
    pub fn vector(&self) -> bool {
        self.vector
    }

    #[must_use]
    pub fn overlay_search(&self) -> bool {
        self.overlay_search
    }

    #[must_use]
    pub fn sync(&self) -> bool {
        self.sync
    }

    #[must_use]
    pub fn encryption(&self) -> bool {
        self.encryption
    }
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
