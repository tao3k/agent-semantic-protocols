//! Runtime Server telemetry uses Tokio for socket I/O, task lifecycle,
//! backpressure, shutdown, filesystem setup, and the Turso writer lane.
//! Standard-library atomics and time values are in-memory data primitives;
//! they never perform blocking lifecycle or filesystem work.

mod exporter;
mod observation;
mod query;
mod query_server;
mod runtime;
pub use runtime::try_emit_to_runtime;
pub use runtime::try_record_to_active_runtime;
mod semconv;

pub use exporter::TursoOpenTelemetrySpanExporter;
pub use observation::RuntimePerformanceObservation;
pub use query::{
    RuntimePerformanceQuery, RuntimePerformanceQueryReceipt, query_runtime_performance,
};
pub use runtime::{RuntimeServerOpenTelemetry, RuntimeServerOpenTelemetryHandle};
