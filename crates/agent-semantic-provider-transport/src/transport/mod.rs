// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Tokio-backed process runner for ASP language providers.

mod output;
mod runtime;

#[path = "../transport_io.rs"]
mod io_tasks;

pub use runtime::ProviderProcessOutput;
pub use runtime::ProviderProcessStarted;
pub use runtime::ProviderProcessSupervisor;
pub use runtime::provider_process_limits_from_environment;

#[cfg(test)]
use crate::ProviderProcessFraming;
use io_tasks::ProviderIoTasks;
use io_tasks::spawn_provider_io_tasks;
use output::collect_provider_output;
use output::kill_provider_process_group;
use output::provider_process_group_id;
use output::write_stdin;
#[cfg(test)]
use runtime::provider_process_admission_slots_for;
#[cfg(test)]
use runtime::spawn_provider_process;

#[cfg(test)]
#[path = "../../tests/unit/transport/mod.rs"]
mod transport_tests;
