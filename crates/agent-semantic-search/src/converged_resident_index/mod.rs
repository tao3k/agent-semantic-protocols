// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! High-performance sub-millisecond converged 4-axis resident workspace search engine
//! with compact 6-byte trigram postings, HIR boolean query planning, zero-copy `mmap`
//! snapshots, and incremental delta updates.

mod build;
mod edb;
mod edb_cache;
mod index;
mod provider_ingest;
mod query;
mod symbols;

pub use edb::{
    SEMANTIC_OPEN_EDB_SCHEMA_ID, SEMANTIC_OPEN_EDB_SCHEMA_VERSION, WorkspaceEdbAttr,
    WorkspaceEdbEdgeFact, WorkspaceEdbEntityFact, WorkspaceEdbFileFact, WorkspaceEdbPrefilter,
    WorkspaceEdbSnapshot, WorkspaceOpenEdbIngestBatch, WorkspaceOpenEdbIngestReceipt,
    WorkspaceOpenEdbPendingFile,
};
pub use edb_cache::{
    OPEN_EDB_CACHE_SCHEMA_VERSION, WorkspaceOpenEdbCache, WorkspaceOpenEdbCacheFile,
    WorkspaceOpenEdbRestoreReceipt,
};
pub use index::{
    WorkspaceContextLine, WorkspaceDeltaReceipt, WorkspaceFileDelta, WorkspaceIndexConfig,
    WorkspaceSearchHit, WorkspaceSearchIndex, WorkspaceSearchQuery, WorkspaceSearchReport,
};
pub use provider_ingest::{
    parse_native_syntax_fact_index_json, parse_provider_projection_batch_json,
};
pub use symbols::{WorkspaceSymbolKind, WorkspaceSymbolMatch};
