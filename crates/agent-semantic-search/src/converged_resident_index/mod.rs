// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! High-performance sub-millisecond converged 4-axis resident workspace search engine
//! with compact 6-byte trigram postings, HIR boolean query planning, zero-copy `mmap`
//! snapshots, and incremental delta updates.

mod build;
mod index;
mod query;

pub use index::{
    WorkspaceContextLine, WorkspaceDeltaReceipt, WorkspaceFileDelta, WorkspaceIndexConfig,
    WorkspaceSearchHit, WorkspaceSearchIndex, WorkspaceSearchQuery, WorkspaceSearchReport,
};
