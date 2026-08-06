//! Runtime Server telemetry uses Tokio for socket I/O, task lifecycle,
//! backpressure, shutdown, filesystem setup, and the Turso writer lane.
//! Standard-library atomics and time values are in-memory data primitives;
//! they never perform blocking lifecycle or filesystem work.

mod exporter;
mod live_store;
mod observation;
mod process_memory;
mod query;
mod query_server;
mod runtime;
pub use runtime::admit_to_runtime;
pub(crate) use runtime::begin_runtime_memory_operation;
pub use runtime::emit_to_runtime;
pub use runtime::try_record_to_active_runtime;
mod semconv;

pub use exporter::{ActiveSearchIncident, TursoOpenTelemetrySpanExporter};
pub use observation::{RuntimeLifecycleEvent, RuntimePerformanceObservation};
pub use query::{
    RuntimePerformanceQuery, RuntimePerformanceQueryReceipt, query_runtime_performance,
};
pub use runtime::{
    RuntimePerformanceIngressReceipt, RuntimeServerOpenTelemetry, RuntimeServerOpenTelemetryHandle,
};
