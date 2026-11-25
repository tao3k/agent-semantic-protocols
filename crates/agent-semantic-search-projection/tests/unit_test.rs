// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Unit-test harness for the search projection kernel.

#[path = "unit/query_playbook_materialization.rs"]
mod query_playbook_materialization;
#[path = "unit/resident_search_result.rs"]
mod resident_search_result;
#[path = "unit/runtime_graph_request.rs"]
mod runtime_graph_request;
#[path = "unit/search_topology_settlement.rs"]
mod search_topology_settlement;
#[path = "unit/source.rs"]
mod source;
#[path = "unit/storage_route.rs"]
mod storage_route;

pub(crate) fn ensure_orgize_native_runtime() {
    // SAFETY: unit tests in this crate are pure in-memory checks that do not
    // spawn subprocesses or alter host stdio/signal state; `initialize_native_runtime`
    // is process-global and idempotent via `OnceLock`.
    unsafe { orgize::initialize_native_runtime() }.expect("native Org parser startup");
}
