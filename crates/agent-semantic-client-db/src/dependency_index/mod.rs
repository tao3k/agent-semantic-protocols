// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-only

//! DB-owned dependency indexes for language runtimes and manifests.

mod gerbil;

pub use gerbil::{
    DEFAULT_GERBIL_DEPS_SEARCH_LIMIT, GerbilDepsQueryRequest, GerbilDepsQueryResult,
    GerbilDepsSearchRequest, GerbilDepsSearchResult, gerbil_deps_minimal_import,
    gerbil_deps_query_export, gerbil_deps_query_terms, gerbil_deps_search_exports,
    gerbil_deps_selector_for, gerbil_deps_validate_module_id, gerbil_deps_validate_symbol,
};
