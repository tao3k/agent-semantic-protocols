// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! High-performance sub-millisecond converged 4-axis resident workspace search engine,
//! upgraded with `microsoft/tgrep`-grade indexing and query planning.

mod build;
mod index;
mod query;

pub use index::{
    WorkspaceContextLine, WorkspaceDeltaReceipt, WorkspaceFileDelta, WorkspaceIndexConfig,
    WorkspaceSearchHit, WorkspaceSearchIndex, WorkspaceSearchQuery, WorkspaceSearchReport,
};
