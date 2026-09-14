// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Lightweight Tokio scheduling and deterministic ownership for ASP runtimes.

mod plan_order;
mod resource_supervisor;
mod runtime_profile;
mod task_scope;

pub use plan_order::join_tasks_in_plan_order;
pub use resource_supervisor::{
    RuntimeServerResourcePermit, RuntimeServerResourcePermitReceipt, RuntimeServerResourceRequest,
    RuntimeServerResourceSupervisor, runtime_server_process_memory_budget_bytes,
};
pub use runtime_profile::{
    RuntimeServerClientExecutor, RuntimeServerRuntime, RuntimeServerRuntimeBuilder,
    adaptive_tokio_worker_count,
};
pub use task_scope::{
    RuntimeServerOwnedTask, RuntimeServerTaskLifecycleReceipt, RuntimeServerTaskPermit,
    RuntimeServerTaskScope,
};

#[cfg(test)]
#[path = "../tests/unit/runtime_server.rs"]
mod runtime_server_tests;
