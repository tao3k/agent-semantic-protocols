// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Runtime process-observation facade.

mod observation;

pub use observation::{
    MEMORY_WATERMARK_STEP_BYTES, ProcessMemoryObservation, RUNTIME_EVENT_LOOP_LAG_BUDGET_MICROS,
    RUNTIME_SERVER_OPEN_DESCRIPTOR_BUDGET, RuntimeSchedulerObservation, linux_statm_resident_bytes,
    observe_process_memory, observe_runtime_scheduler, run_sampler,
};

#[cfg(test)]
#[path = "../tests/unit/observation.rs"]
mod observation_tests;
