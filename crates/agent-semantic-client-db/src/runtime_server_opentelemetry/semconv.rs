//! Generated Rust projection of
//! `schemas/telemetry/model/asp-performance.yaml`.

pub(super) const SURFACE: &str = "asp.surface";
pub(super) const STAGE: &str = "asp.stage";
pub(super) const WORKSPACE_IDENTITY: &str = "asp.workspace.identity";
pub(super) const LANGUAGE_ID: &str = "asp.language.id";
pub(super) const GENERATION_DIGEST: &str = "asp.generation.digest";
pub(super) const RUNTIME_ARTIFACT_DIGEST: &str = "asp.runtime.artifact.digest";
pub(super) const TRANSPORT_CONTRACT_DIGEST: &str = "asp.runtime.transport_contract.digest";
pub(super) const OPERATION_ID: &str = "asp.operation.id";
pub(super) const EVENT_IDENTITY: &str = "asp.performance.event.identity";
pub(super) const INCIDENT_ID: &str = "asp.search.incident.id";
pub(super) const INCIDENT_STATE: &str = "asp.search.incident.state";
pub(super) const INCIDENT_TRANSITION: &str = "asp.search.incident.transition";
pub(super) const INCIDENT_TRANSITION_SEQUENCE: &str = "asp.search.incident.transition_sequence";
pub(super) const REQUESTED_PROJECTION: &str = "asp.search.requested_projection";
pub(super) const OBSERVED_AT_UNIX_MICROS: &str = "asp.performance.observed_at_unix_micros";
pub(super) const PROCESS_RESIDENT_MEMORY: &str = "asp.process.resident_memory";
pub(super) const PROCESS_PEAK_RESIDENT_MEMORY: &str = "asp.process.peak_resident_memory";
pub(super) const PROCESS_MEMORY_BUDGET: &str = "asp.process.memory.budget";
pub(super) const PROCESS_MEMORY_BUDGET_STATUS: &str = "asp.process.memory.budget.status";
pub(super) const PROCESS_DISK_READ_BYTES: &str = "asp.process.disk.read_bytes";
pub(super) const PROCESS_DISK_WRITE_BYTES: &str = "asp.process.disk.write_bytes";
pub(super) const PROCESS_PAGE_INS: &str = "asp.process.page_ins";
pub(super) const PROCESS_OPEN_DESCRIPTORS: &str = "asp.process.open_descriptors";
pub(super) const PROCESS_OPEN_DESCRIPTOR_BUDGET: &str = "asp.process.open_descriptors.budget";
pub(super) const PROCESS_OPEN_DESCRIPTOR_BUDGET_STATUS: &str =
    "asp.process.open_descriptors.budget.status";
pub(super) const RUNTIME_EVENT_LOOP_LAG: &str = "asp.runtime.event_loop.lag";
pub(super) const RUNTIME_WORKER_THREADS: &str = "asp.runtime.worker_threads";
pub(super) const RUNTIME_ALIVE_TASKS: &str = "asp.runtime.alive_tasks";
pub(super) const RUNTIME_GLOBAL_QUEUE_DEPTH: &str = "asp.runtime.global_queue_depth";
pub(super) const RUNTIME_ACTIVE_CONNECTIONS: &str = "asp.runtime.active_connections";
pub(super) const RUNTIME_CONNECTION_LIMIT: &str = "asp.runtime.connection_limit";
pub(super) const RUNTIME_CONNECTION_HIGH_WATERMARK: &str = "asp.runtime.connection_high_watermark";
pub(super) const RUNTIME_REJECTED_CONNECTIONS: &str = "asp.runtime.rejected_connections";
pub(super) const RUNTIME_DIAGNOSTIC_QUEUE_DEPTH: &str = "asp.runtime.diagnostic_queue_depth";
pub(super) const RUNTIME_DIAGNOSTIC_QUEUE_CAPACITY: &str = "asp.runtime.diagnostic_queue_capacity";
pub(super) const RUNTIME_DROPPED_DIAGNOSTICS: &str = "asp.runtime.dropped_diagnostics";
pub(super) const RUNTIME_EVENT_LOOP_LAG_BUDGET: &str = "asp.runtime.event_loop.lag.budget";
pub(super) const RUNTIME_EVENT_LOOP_LAG_BUDGET_STATUS: &str =
    "asp.runtime.event_loop.lag.budget.status";
pub(super) const OWNER_COUNT: &str = "asp.materialization.owner.count";
pub(super) const SELECTOR_COUNT: &str = "asp.materialization.selector.count";
pub(super) const RELATION_COUNT: &str = "asp.materialization.relation.count";
pub(super) const SOURCE_BYTES: &str = "asp.materialization.source.bytes";
pub(super) const PROJECTION_BYTES: &str = "asp.materialization.projection.bytes";
pub(super) const CANONICAL_ENCODED_BYTES: &str = "asp.materialization.canonical.encoded.bytes";
pub(super) const ELAPSED_MICROS: &str = "asp.performance.elapsed";
pub(super) const BUDGET_MICROS: &str = "asp.performance.budget";
pub(super) const BUDGET_STATUS: &str = "asp.performance.budget.status";
pub(super) const FAILURE_REASON: &str = "asp.failure.reason";
pub(super) const RETRY_AFTER_MS: &str = "asp.retry_after";
