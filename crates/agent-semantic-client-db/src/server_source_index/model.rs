// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Public data model for ASP Server-owned source-index refresh and lookup receipts.

pub use crate::{
    ClientDbSourceIndexCandidate as SourceIndexCandidate,
    ClientDbSourceIndexLookupResult as SourceIndexLookupResult,
    ClientDbSourceIndexLookupState as SourceIndexLookupState,
    ClientDbSourceIndexRefreshResult as SourceIndexRefreshReport,
    ClientDbSourceIndexScopeFile as SourceIndexScopeFile,
    ClientDbSourceIndexSourceKind as SourceIndexSourceKind,
};
