// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Runtime Server-owned telemetry uses Tokio for socket I/O, task lifecycle,
//! backpressure, shutdown, filesystem setup, and the Turso writer lane.
//! Standard-library atomics and time values are in-memory data primitives;
//! they never perform blocking lifecycle or filesystem work.

mod exporter;
mod handle;
mod live_store;
mod query;
mod query_model;
mod query_server;
mod runtime;
pub use runtime::admit_to_runtime;
pub use runtime::emit_to_runtime;
mod semconv;

pub use exporter::{ActiveSearchIncident, TursoOpenTelemetrySpanExporter};
pub use handle::RuntimeServerOpenTelemetryHandle;
pub use query::{
    RuntimePerformanceQuery, RuntimePerformanceQueryReceipt, query_runtime_performance,
};
pub use runtime::{RuntimePerformanceIngressReceipt, RuntimeServerOpenTelemetry};
