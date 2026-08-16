//! Tokio-backed process runner for ASP language providers.

mod output;
mod runtime;

#[path = "../transport_io.rs"]
mod io_tasks;

pub use runtime::{
    ProviderProcessOutput, ProviderProcessSupervisor, provider_process_limits_from_environment,
};

#[cfg(test)]
use crate::ProviderProcessFraming;
use io_tasks::{ProviderIoTasks, spawn_provider_io_tasks};
use output::{
    collect_provider_output, kill_provider_process_group, provider_process_group_id, write_stdin,
};
#[cfg(test)]
use runtime::{provider_process_admission_slots_for, spawn_provider_process};

#[cfg(test)]
#[path = "../../tests/unit/transport/mod.rs"]
mod transport_tests;
