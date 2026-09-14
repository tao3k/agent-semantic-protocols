// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Neutral Version 1 Runtime observation facade. Telemetry values and the
//! process-local resident sink remain separate owner modules.

mod sink;
mod telemetry;

pub use sink::{
    RuntimeMemoryOperationGuard, RuntimeObservationSink, RuntimeObservationSinkRegistration,
    active_runtime_memory_operations, begin_runtime_memory_operation,
    register_runtime_observation_sink, try_record_to_active_runtime,
};
pub use telemetry::{
    RUNTIME_SEARCH_TELEMETRY_PHASES, RUNTIME_SEARCH_TELEMETRY_SCHEMA_REF, RuntimeLifecycleEvent,
    RuntimePerformanceObservation, RuntimeSearchTelemetryArtifact, RuntimeSearchTelemetryCollector,
    RuntimeSearchTelemetryError, RuntimeSearchTelemetryIdentity,
    RuntimeSearchTelemetryIdentityInput, RuntimeSearchTelemetryTrace,
};

#[cfg(test)]
#[path = "../tests/unit/sink.rs"]
mod sink_tests;
