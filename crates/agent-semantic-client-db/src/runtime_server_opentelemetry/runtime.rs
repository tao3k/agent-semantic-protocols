use std::{
    collections::{BTreeMap, HashMap},
    path::Path,
    sync::Arc,
};

use opentelemetry::{
    KeyValue,
    trace::{Span, Status, Tracer, TracerProvider},
};
use opentelemetry_sdk::{
    runtime,
    trace::{SdkTracerProvider, span_processor_with_async_runtime::BatchSpanProcessor},
};
use tokio::sync::{mpsc, watch};

use super::{
    exporter::TursoOpenTelemetrySpanExporter, observation::RuntimePerformanceObservation, semconv,
};

#[derive(Clone, Debug)]
pub struct RuntimeServerOpenTelemetryHandle {
    sender: mpsc::Sender<RuntimePerformanceObservation>,
    dropped_observations: Arc<std::sync::atomic::AtomicU64>,
}

impl RuntimeServerOpenTelemetryHandle {
    pub fn try_record(&self, observation: RuntimePerformanceObservation) -> bool {
        match self.sender.try_send(observation) {
            Ok(()) => true,
            Err(_) => {
                self.dropped_observations
                    .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                false
            }
        }
    }

    pub fn dropped_observation_count(&self) -> u64 {
        self.dropped_observations
            .load(std::sync::atomic::Ordering::Relaxed)
    }
}

pub struct RuntimeServerOpenTelemetry {
    handle: RuntimeServerOpenTelemetryHandle,
    registration_id: u64,
    shutdown: watch::Sender<bool>,
    task: tokio::task::JoinHandle<Result<(), String>>,
}

static ACTIVE_RUNTIME_TELEMETRY: std::sync::OnceLock<
    std::sync::Mutex<Option<(u64, RuntimeServerOpenTelemetryHandle)>>,
> = std::sync::OnceLock::new();
static NEXT_RUNTIME_TELEMETRY_REGISTRATION_ID: std::sync::atomic::AtomicU64 =
    std::sync::atomic::AtomicU64::new(1);
static ACTIVE_MEMORY_OPERATIONS: std::sync::OnceLock<std::sync::Mutex<BTreeMap<String, String>>> =
    std::sync::OnceLock::new();

pub(crate) struct RuntimeMemoryOperationGuard {
    operation_id: String,
}

impl Drop for RuntimeMemoryOperationGuard {
    fn drop(&mut self) {
        if let Some(active) = ACTIVE_MEMORY_OPERATIONS.get()
            && let Ok(mut active) = active.lock()
        {
            active.remove(&self.operation_id);
        }
    }
}

pub(crate) fn begin_runtime_memory_operation(
    workspace_identity: impl Into<String>,
    operation_id: impl Into<String>,
) -> RuntimeMemoryOperationGuard {
    let operation_id = operation_id.into();
    if let Ok(mut active) = ACTIVE_MEMORY_OPERATIONS
        .get_or_init(|| std::sync::Mutex::new(BTreeMap::new()))
        .lock()
    {
        active.insert(operation_id.clone(), workspace_identity.into());
    }
    RuntimeMemoryOperationGuard { operation_id }
}

fn active_memory_operations() -> Vec<(String, String)> {
    ACTIVE_MEMORY_OPERATIONS
        .get()
        .and_then(|active| active.lock().ok())
        .map(|active| {
            active
                .iter()
                .map(|(operation_id, workspace_identity)| {
                    (operation_id.clone(), workspace_identity.clone())
                })
                .collect()
        })
        .unwrap_or_default()
}

pub fn try_record_to_active_runtime(observation: RuntimePerformanceObservation) -> bool {
    let Some(active) = ACTIVE_RUNTIME_TELEMETRY.get() else {
        return false;
    };
    let Ok(active) = active.lock() else {
        return false;
    };
    active
        .as_ref()
        .is_some_and(|(_, handle)| handle.try_record(observation))
}

impl RuntimeServerOpenTelemetry {
    /// Binds the resident telemetry surfaces only after the initial process and
    /// Tokio scheduler observation is available. Turso open and schema
    /// bootstrap remain resident-task work.
    pub async fn start(
        database_path: std::path::PathBuf,
        ingress_socket_path: std::path::PathBuf,
        query_socket_path: std::path::PathBuf,
    ) -> Result<Self, String> {
        let scheduler_probe_started = tokio::time::Instant::now();
        tokio::task::yield_now().await;
        let event_loop_lag_micros =
            u64::try_from(scheduler_probe_started.elapsed().as_micros()).unwrap_or(u64::MAX);
        let initial_process_memory = tokio::task::spawn_blocking(move || {
            super::process_memory::observe_process_memory(event_loop_lag_micros)
        })
        .await
        .map_err(|error| {
            format!("initial Runtime Server process-memory probe task failed: {error}")
        })?;
        let worker_count = tokio::runtime::Handle::current().metrics().num_workers();
        let capacity = worker_count.saturating_mul(64).clamp(64, 4096);
        let (sender, receiver) = mpsc::channel(capacity);
        let (shutdown, shutdown_receiver) = watch::channel(false);
        let dropped_observations = Arc::new(std::sync::atomic::AtomicU64::new(0));
        let ingress = tokio::net::UnixDatagram::bind(&ingress_socket_path).map_err(|error| {
            format!(
                "failed to bind Runtime Server OpenTelemetry ingress {}: {error}",
                ingress_socket_path.display()
            )
        })?;
        let query_listener =
            tokio::net::UnixListener::bind(&query_socket_path).map_err(|error| {
                format!(
                    "failed to bind Runtime Server OpenTelemetry query {}: {error}",
                    query_socket_path.display()
                )
            })?;
        let handle = RuntimeServerOpenTelemetryHandle {
            sender,
            dropped_observations: Arc::clone(&dropped_observations),
        };
        let registration_id = NEXT_RUNTIME_TELEMETRY_REGISTRATION_ID
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let active = ACTIVE_RUNTIME_TELEMETRY.get_or_init(|| std::sync::Mutex::new(None));
        *active
            .lock()
            .map_err(|_| "Runtime Server OpenTelemetry registry lock poisoned".to_owned())? =
            Some((registration_id, handle.clone()));
        let task = tokio::spawn(run_resident_telemetry_lane(
            database_path,
            receiver,
            ingress,
            query_listener,
            dropped_observations,
            initial_process_memory,
            shutdown_receiver,
        ));
        Ok(Self {
            handle,
            registration_id,
            shutdown,
            task,
        })
    }

    pub fn handle(&self) -> RuntimeServerOpenTelemetryHandle {
        self.handle.clone()
    }

    pub async fn shutdown(self) -> Result<(), String> {
        if let Some(active) = ACTIVE_RUNTIME_TELEMETRY.get()
            && let Ok(mut active) = active.lock()
            && active
                .as_ref()
                .is_some_and(|(registration_id, _)| *registration_id == self.registration_id)
        {
            *active = None;
        }
        let _ = self.shutdown.send(true);
        drop(self.handle);
        self.task
            .await
            .map_err(|error| format!("Runtime Server OpenTelemetry task failed: {error}"))?
    }
}

/// Emits one observation through Tokio to the resident telemetry lane.
pub async fn emit_to_runtime(
    ingress_socket_path: &Path,
    observation: &RuntimePerformanceObservation,
) -> bool {
    let Ok(bytes) = serde_json::to_vec(observation) else {
        return false;
    };
    let Ok(socket) = tokio::net::UnixDatagram::unbound() else {
        return false;
    };
    socket.send_to(&bytes, ingress_socket_path).await.is_ok()
}

async fn run_resident_telemetry_lane(
    database_path: std::path::PathBuf,
    mut receiver: mpsc::Receiver<RuntimePerformanceObservation>,
    ingress: tokio::net::UnixDatagram,
    query_listener: tokio::net::UnixListener,
    dropped_observations: Arc<std::sync::atomic::AtomicU64>,
    initial_process_memory: Option<super::process_memory::ProcessMemoryObservation>,
    mut shutdown: watch::Receiver<bool>,
) -> Result<(), String> {
    let exporter = TursoOpenTelemetrySpanExporter::open(&database_path).await?;
    let processor = BatchSpanProcessor::builder(exporter.clone(), runtime::Tokio).build();
    let provider = SdkTracerProvider::builder()
        .with_span_processor(processor)
        .build();
    let (query_shutdown, query_shutdown_receiver) = watch::channel(false);
    let query_task = tokio::spawn(super::query_server::run_query_server(
        query_listener,
        exporter,
        provider.clone(),
        query_shutdown_receiver,
    ));
    let tracer = provider.tracer("asp.runtime-server.performance");
    let mut ingress_buffer = vec![0_u8; 16 * 1024];
    let (memory_sender, mut memory_receiver) = watch::channel(initial_process_memory);
    let mut memory_sampler =
        tokio::spawn(run_process_memory_sampler(memory_sender, shutdown.clone()));
    let mut latest_process_memory = initial_process_memory;
    let mut recorded_memory_watermarks = HashMap::<String, u64>::new();
    loop {
        tokio::select! {
            biased;
            changed = shutdown.changed() => {
                let _ = changed;
                while let Ok(observation) = receiver.try_recv() {
                    record_observation_with_memory(
                        &tracer,
                        observation,
                        latest_process_memory,
                    );
                }
                break;
            }
            observation = receiver.recv() => match observation {
                Some(observation) => record_observation_with_memory(
                    &tracer,
                    observation,
                    latest_process_memory,
                ),
                None => break,
            },
            ingress_result = ingress.recv(&mut ingress_buffer) => match ingress_result {
                Ok(length) => match serde_json::from_slice::<RuntimePerformanceObservation>(
                    &ingress_buffer[..length],
                ) {
                    Ok(observation) => record_observation_with_memory(
                        &tracer,
                        observation,
                        latest_process_memory,
                    ),
                    Err(_) => {
                        dropped_observations.fetch_add(
                            1,
                            std::sync::atomic::Ordering::Relaxed,
                        );
                    }
                },
                Err(_) => {
                    dropped_observations.fetch_add(
                        1,
                        std::sync::atomic::Ordering::Relaxed,
                    );
                }
            },
            changed = memory_receiver.changed() => {
                if changed.is_err() {
                    return Err("Runtime Server process-memory sampler closed unexpectedly".to_owned());
                }
                latest_process_memory = *memory_receiver.borrow_and_update();
                let active_operations = active_memory_operations();
                recorded_memory_watermarks.retain(|operation_id, _| {
                    active_operations.iter().any(|(active_id, _)| active_id == operation_id)
                });
                for (operation_id, workspace_identity) in active_operations {
                    let mut observation = RuntimePerformanceObservation::new(
                        "runtime-server",
                        "process-memory-watermark",
                        0,
                        0,
                        "within-budget",
                    )
                    .with_operation_id(operation_id.clone());
                    observation.workspace_identity = Some(workspace_identity);
                    if let Some(memory) = latest_process_memory {
                        observation.record_runtime_process_memory(memory);
                    }
                    let observed_peak = observation
                        .process_peak_resident_bytes
                        .or(observation.process_resident_bytes);
                    if let Some(observed_peak) = observed_peak {
                        let watermark = observed_peak
                            / super::process_memory::MEMORY_WATERMARK_STEP_BYTES;
                        let should_record = recorded_memory_watermarks
                            .get(&operation_id)
                            .is_none_or(|recorded| watermark > *recorded);
                        if should_record {
                            recorded_memory_watermarks.insert(operation_id, watermark);
                            record_observation(&tracer, observation);
                        }
                    }
                }
            },
            sampler_result = &mut memory_sampler => {
                return sampler_result
                    .map_err(|error| format!("Runtime Server process-memory sampler task failed: {error}"))?;
            },
        }
    }
    memory_sampler
        .await
        .map_err(|error| format!("Runtime Server process-memory sampler task failed: {error}"))??;
    let _ = query_shutdown.send(true);
    query_task
        .await
        .map_err(|error| format!("Runtime Server telemetry query server failed: {error}"))??;
    tokio::task::spawn_blocking(move || provider.shutdown())
        .await
        .map_err(|error| format!("OpenTelemetry provider shutdown task failed: {error}"))?
        .map_err(|error| format!("OpenTelemetry provider shutdown failed: {error}"))
}

async fn run_process_memory_sampler(
    memory: watch::Sender<Option<super::process_memory::ProcessMemoryObservation>>,
    mut shutdown: watch::Receiver<bool>,
) -> Result<(), String> {
    let mut interval = tokio::time::interval(std::time::Duration::from_millis(250));
    interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    loop {
        tokio::select! {
            scheduled = interval.tick() => {
                let event_loop_lag_micros = u64::try_from(
                    tokio::time::Instant::now()
                        .saturating_duration_since(scheduled)
                        .as_micros(),
                )
                .unwrap_or(u64::MAX);
                let observation = tokio::task::spawn_blocking(
                    move || super::process_memory::observe_process_memory(event_loop_lag_micros),
                )
                .await
                .map_err(|error| {
                    format!("Runtime Server process-memory probe task failed: {error}")
                })?;
                memory.send_replace(observation);
            }
            changed = shutdown.changed() => {
                let _ = changed;
                return Ok(());
            }
        }
    }
}

fn record_observation_with_memory(
    tracer: &opentelemetry_sdk::trace::SdkTracer,
    mut observation: RuntimePerformanceObservation,
    memory: Option<super::process_memory::ProcessMemoryObservation>,
) {
    if let Some(memory) = memory {
        observation.record_runtime_process_memory(memory);
    }
    record_observation(tracer, observation);
}

fn record_observation(
    tracer: &opentelemetry_sdk::trace::SdkTracer,
    observation: RuntimePerformanceObservation,
) {
    let mut span = tracer.start(format!("asp {} {}", observation.surface, observation.stage));
    if observation.budget_status == "budget-exceeded" {
        span.set_status(Status::error("performance-budget-exceeded"));
    }
    let mut attributes = vec![
        KeyValue::new(semconv::SURFACE, observation.surface),
        KeyValue::new(semconv::STAGE, observation.stage),
        KeyValue::new(
            semconv::ELAPSED_MICROS,
            i64::try_from(observation.elapsed_micros).unwrap_or(i64::MAX),
        ),
        KeyValue::new(
            semconv::BUDGET_MICROS,
            i64::try_from(observation.budget_micros).unwrap_or(i64::MAX),
        ),
        KeyValue::new(semconv::BUDGET_STATUS, observation.budget_status),
    ];
    push_optional(
        &mut attributes,
        semconv::WORKSPACE_IDENTITY,
        observation.workspace_identity,
    );
    push_optional(
        &mut attributes,
        semconv::LANGUAGE_ID,
        observation.language_id,
    );
    push_optional(
        &mut attributes,
        semconv::GENERATION_DIGEST,
        observation.generation_digest,
    );
    push_optional(
        &mut attributes,
        semconv::RUNTIME_ARTIFACT_DIGEST,
        observation.runtime_artifact_digest,
    );
    push_optional(
        &mut attributes,
        semconv::TRANSPORT_CONTRACT_DIGEST,
        observation.transport_contract_digest,
    );
    push_optional(
        &mut attributes,
        semconv::OPERATION_ID,
        observation.operation_id,
    );
    push_optional_u64(
        &mut attributes,
        semconv::PROCESS_RESIDENT_MEMORY,
        observation.process_resident_bytes,
    );
    push_optional_u64(
        &mut attributes,
        semconv::PROCESS_PEAK_RESIDENT_MEMORY,
        observation.process_peak_resident_bytes,
    );
    push_optional_u64(
        &mut attributes,
        semconv::PROCESS_MEMORY_BUDGET,
        observation.process_memory_budget_bytes,
    );
    push_optional(
        &mut attributes,
        semconv::PROCESS_MEMORY_BUDGET_STATUS,
        observation.process_memory_budget_status,
    );
    push_optional_u64(
        &mut attributes,
        semconv::PROCESS_DISK_READ_BYTES,
        observation.process_disk_read_bytes,
    );
    push_optional_u64(
        &mut attributes,
        semconv::PROCESS_DISK_WRITE_BYTES,
        observation.process_disk_write_bytes,
    );
    push_optional_u64(
        &mut attributes,
        semconv::PROCESS_PAGE_INS,
        observation.process_page_ins,
    );
    push_optional_u64(
        &mut attributes,
        semconv::RUNTIME_EVENT_LOOP_LAG,
        observation.runtime_event_loop_lag_micros,
    );
    push_optional_u64(
        &mut attributes,
        semconv::RUNTIME_EVENT_LOOP_LAG_BUDGET,
        observation.runtime_event_loop_lag_budget_micros,
    );
    push_optional(
        &mut attributes,
        semconv::RUNTIME_EVENT_LOOP_LAG_BUDGET_STATUS,
        observation.runtime_event_loop_lag_budget_status,
    );
    push_optional_u64(
        &mut attributes,
        semconv::OWNER_COUNT,
        observation.owner_count,
    );
    push_optional_u64(
        &mut attributes,
        semconv::SELECTOR_COUNT,
        observation.selector_count,
    );
    push_optional_u64(
        &mut attributes,
        semconv::RELATION_COUNT,
        observation.relation_count,
    );
    push_optional_u64(
        &mut attributes,
        semconv::SOURCE_BYTES,
        observation.source_bytes,
    );
    push_optional_u64(
        &mut attributes,
        semconv::PROJECTION_BYTES,
        observation.projection_bytes,
    );
    push_optional_u64(
        &mut attributes,
        semconv::CANONICAL_ENCODED_BYTES,
        observation.canonical_encoded_bytes,
    );
    if let Some(reason) = observation.failure_reason {
        attributes.push(KeyValue::new(semconv::FAILURE_REASON, reason.clone()));
        span.set_status(Status::error(reason));
    }
    if let Some(retry_after_ms) = observation.retry_after_ms {
        attributes.push(KeyValue::new(
            semconv::RETRY_AFTER_MS,
            i64::try_from(retry_after_ms).unwrap_or(i64::MAX),
        ));
    }
    span.set_attributes(attributes);
    span.end();
}

fn push_optional(attributes: &mut Vec<KeyValue>, key: &'static str, value: Option<String>) {
    if let Some(value) = value.filter(|value| !value.is_empty()) {
        attributes.push(KeyValue::new(key, value));
    }
}

fn push_optional_u64(attributes: &mut Vec<KeyValue>, key: &'static str, value: Option<u64>) {
    if let Some(value) = value {
        attributes.push(KeyValue::new(key, i64::try_from(value).unwrap_or(i64::MAX)));
    }
}
