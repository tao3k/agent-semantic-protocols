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
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    sync::{mpsc, watch},
};

use super::{
    exporter::TursoOpenTelemetrySpanExporter,
    observation::{RuntimeLifecycleEvent, RuntimePerformanceObservation},
    semconv,
};

#[derive(Clone, Debug)]
pub struct RuntimeServerOpenTelemetryHandle {
    sender: mpsc::Sender<RuntimePerformanceObservation>,
    dropped_observations: Arc<std::sync::atomic::AtomicU64>,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RuntimePerformanceIngressReceipt {
    schema_id: String,
    schema_version: String,
    pub state: String,
    pub workspace_identity: String,
    pub surface: String,
    pub stage: String,
    pub event_identity: Option<String>,
}

impl RuntimeServerOpenTelemetryHandle {
    pub fn try_record_lifecycle(&self, event: RuntimeLifecycleEvent) -> bool {
        self.try_record(event.into_observation())
    }
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
    task: crate::runtime_server_runtime::RuntimeServerOwnedTask<Result<(), String>>,
    telemetry_task: crate::runtime_server_runtime::RuntimeServerOwnedTask<Result<(), String>>,
    task_scope: crate::runtime_server_runtime::RuntimeServerTaskScope,
    ingress_socket_path: std::path::PathBuf,
    query_socket_path: std::path::PathBuf,
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
        let bus = crate::runtime_telemetry_bus::RuntimeTelemetryBus::new();
        Self::start_with_telemetry_receiver(
            database_path,
            ingress_socket_path,
            query_socket_path,
            bus.receiver,
        )
        .await
    }

    pub async fn start_with_telemetry_receiver(
        database_path: std::path::PathBuf,
        ingress_socket_path: std::path::PathBuf,
        query_socket_path: std::path::PathBuf,
        mut telemetry_receiver: crate::runtime_telemetry_bus::RuntimeTelemetryBusReceiver,
    ) -> Result<Self, String> {
        let scheduler_probe_started = tokio::time::Instant::now();
        tokio::task::yield_now().await;
        let event_loop_lag_micros =
            u64::try_from(scheduler_probe_started.elapsed().as_micros()).unwrap_or(u64::MAX);
        let scheduler = super::process_memory::observe_runtime_scheduler();
        let initial_process_memory = tokio::task::spawn_blocking(move || {
            super::process_memory::observe_process_memory(event_loop_lag_micros, scheduler)
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
        let ingress = tokio::net::UnixListener::bind(&ingress_socket_path).map_err(|error| {
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
        let task_scope = crate::runtime_server_runtime::RuntimeServerTaskScope::new(
            "runtime-server-opentelemetry",
        );
        let telemetry_sender = handle.sender.clone();
        let mut telemetry_shutdown = shutdown_receiver.clone();
        let telemetry_task = task_scope.spawn("runtime-server-telemetry-bridge", async move {
            loop {
                tokio::select! {
                    biased;
                    _ = telemetry_shutdown.changed() => {
                        telemetry_receiver.close();
                        while let Some(event) = telemetry_receiver.recv().await {
                            if telemetry_sender.send(event.into_observation()).await.is_err() { break; }
                        }
                        break;
                    }
                    event = telemetry_receiver.recv() => match event {
                        Some(event) => {
                            if telemetry_sender.send(event.into_observation()).await.is_err() { break; }
                        }
                        None => break,
                    }
                }
            }
            Ok(())
        })?;
        let lane_task_scope = task_scope.clone();
        let task = task_scope.spawn(
            "runtime-server-opentelemetry-root",
            run_resident_telemetry_lane(
                database_path,
                receiver,
                ingress,
                query_listener,
                dropped_observations,
                initial_process_memory,
                shutdown_receiver,
                lane_task_scope,
            ),
        )?;
        Ok(Self {
            handle,
            registration_id,
            shutdown,
            task,
            telemetry_task,
            task_scope,
            ingress_socket_path,
            query_socket_path,
        })
    }

    pub fn handle(&self) -> RuntimeServerOpenTelemetryHandle {
        self.handle.clone()
    }

    pub async fn shutdown(
        self,
    ) -> Result<crate::runtime_server_runtime::RuntimeServerTaskLifecycleReceipt, String> {
        let drain_started = tokio::time::Instant::now();
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
        self.telemetry_task.join().await??;
        self.task.join().await??;
        remove_socket_if_present(&self.ingress_socket_path).await?;
        remove_socket_if_present(&self.query_socket_path).await?;
        let drain_micros = u64::try_from(drain_started.elapsed().as_micros()).unwrap_or(u64::MAX);
        self.task_scope.finish(drain_micros)
    }
}

/// Emits one observation through Tokio to the resident telemetry lane.
pub async fn emit_to_runtime(
    ingress_socket_path: &Path,
    observation: &RuntimePerformanceObservation,
) -> bool {
    admit_to_runtime(ingress_socket_path, observation)
        .await
        .is_ok()
}

pub async fn admit_to_runtime(
    ingress_socket_path: &Path,
    observation: &RuntimePerformanceObservation,
) -> Result<RuntimePerformanceIngressReceipt, String> {
    let Ok(mut bytes) = serde_json::to_vec(observation) else {
        return Err("failed to encode Runtime Server telemetry observation".to_owned());
    };
    bytes.push(b'\n');
    let Ok(mut stream) = tokio::net::UnixStream::connect(ingress_socket_path).await else {
        return Err("failed to connect Runtime Server telemetry ingress".to_owned());
    };
    if stream.write_all(&bytes).await.is_err() {
        return Err("failed to write Runtime Server telemetry ingress".to_owned());
    }
    let receipt = read_ingress_frame(&mut stream).await?;
    let receipt: RuntimePerformanceIngressReceipt = serde_json::from_slice(&receipt)
        .map_err(|error| format!("failed to decode Runtime Server telemetry receipt: {error}"))?;
    let expected_workspace = observation
        .workspace_identity
        .as_deref()
        .unwrap_or_default();
    if receipt.schema_id != "agent.semantic-protocols.runtime-server-performance-ingress-receipt"
        || receipt.schema_version != "1"
        || !matches!(receipt.state.as_str(), "recorded" | "duplicate")
        || receipt.workspace_identity != expected_workspace
        || receipt.surface != observation.surface
        || receipt.stage != observation.stage
        || receipt.event_identity != observation.event_identity
    {
        return Err(
            "Runtime Server telemetry receipt does not match the submitted canonical key"
                .to_owned(),
        );
    }
    Ok(receipt)
}

async fn run_resident_telemetry_lane(
    database_path: std::path::PathBuf,
    mut receiver: mpsc::Receiver<RuntimePerformanceObservation>,
    ingress: tokio::net::UnixListener,
    query_listener: tokio::net::UnixListener,
    dropped_observations: Arc<std::sync::atomic::AtomicU64>,
    initial_process_memory: Option<super::process_memory::ProcessMemoryObservation>,
    mut shutdown: watch::Receiver<bool>,
    task_scope: crate::runtime_server_runtime::RuntimeServerTaskScope,
) -> Result<(), String> {
    let (provider, persistence_failure) =
        match TursoOpenTelemetrySpanExporter::open(&database_path).await {
            Ok(exporter) => {
                let processor = BatchSpanProcessor::builder(exporter, runtime::Tokio).build();
                (
                    SdkTracerProvider::builder()
                        .with_span_processor(processor)
                        .build(),
                    None,
                )
            }
            Err(error) => (SdkTracerProvider::builder().build(), Some(error)),
        };
    let live_store = std::sync::Arc::new(super::live_store::RuntimePerformanceLiveStore::default());
    let (query_shutdown, query_shutdown_receiver) = watch::channel(false);
    let query_task = task_scope.spawn(
        "runtime-server-telemetry-query",
        super::query_server::run_query_server(
            query_listener,
            std::sync::Arc::clone(&live_store),
            query_shutdown_receiver,
            task_scope.clone(),
        ),
    )?;
    let tracer = provider.tracer("asp.runtime-server.performance");
    if let Some(error) = persistence_failure {
        let mut observation = RuntimePerformanceObservation::new(
            "runtime-server",
            "opentelemetry-persistence-bootstrap",
            0,
            0,
            "unavailable",
        );
        observation.failure_reason = Some("opentelemetry-persistence-unavailable".to_owned());
        record_observation(&tracer, observation, &live_store);
        eprintln!(
            "[runtime-server-opentelemetry] {}",
            serde_json::json!({
                "schemaId": "agent.semantic-protocols.runtime-server-opentelemetry-degradation-receipt",
                "schemaVersion": "1",
                "state": "degraded",
                "reasonKind": "opentelemetry-persistence-unavailable",
                "error": error,
            })
        );
    }
    let mut ingress_connections = tokio::task::JoinSet::new();
    let ingress_supervisor =
        crate::runtime_server_runtime::RuntimeServerConnectionSupervisor::for_current_runtime(
            "runtime-server-telemetry-ingress",
        );
    let (memory_sender, mut memory_receiver) = watch::channel(initial_process_memory);
    let memory_sampler = task_scope.spawn(
        "runtime-server-process-memory-sampler",
        run_process_memory_sampler(memory_sender, shutdown.clone()),
    )?;
    let mut latest_process_memory = initial_process_memory;
    if let Some(memory) = initial_process_memory {
        let mut observation = RuntimePerformanceObservation::new(
            "runtime-server",
            "process-memory-sample",
            0,
            1,
            "within-budget",
        );
        observation.record_runtime_process_memory(memory);
        record_observation(&tracer, observation, &live_store);
    }
    let mut memory_observation_ticks = 0_u64;
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
                        &live_store,
                    );
                }
                break;
            }
            observation = receiver.recv() => match observation {
                Some(observation) => {
                    record_observation_with_memory(
                        &tracer,
                        observation,
                        latest_process_memory,
                        &live_store,
                    );
                }
                None => break,
            },
            accepted = ingress.accept(), if ingress_supervisor.has_capacity() => match accepted {
                Ok((stream, _)) => {
                    let permit = task_scope.permit("runtime-server-telemetry-ingress-connection")?;
                    let connection_lease = ingress_supervisor
                        .try_admit()
                        .expect("capacity guard must admit one telemetry connection");
                    ingress_connections.spawn(async move {
                        let result = crate::runtime_server_runtime::within_connection_io_budget(
                            "telemetry ingress frame",
                            read_ingress_observation(stream),
                        )
                        .await;
                        if result.is_ok() {
                            permit.complete();
                        } else {
                            permit.fail();
                        }
                        (connection_lease, result)
                    });
                }
                Err(_) => {
                    dropped_observations.fetch_add(
                        1,
                        std::sync::atomic::Ordering::Relaxed,
                    );
                }
            },
            completed = ingress_connections.join_next(), if !ingress_connections.is_empty() => {
                if let Some(completed) = completed {
                    commit_ingress_completion(
                        completed,
                        &tracer,
                        latest_process_memory,
                        &live_store,
                        &dropped_observations,
                    )
                    .await;
                }
            },
            changed = memory_receiver.changed() => {
                if changed.is_err() {
                    return Err("Runtime Server process-memory sampler closed unexpectedly".to_owned());
                }
                latest_process_memory = *memory_receiver.borrow_and_update();
                memory_observation_ticks = memory_observation_ticks.saturating_add(1);
                if memory_observation_ticks.is_multiple_of(4)
                    && let Some(memory) = latest_process_memory
                {
                    let mut observation = RuntimePerformanceObservation::new(
                        "runtime-server",
                        "process-memory-sample",
                        0,
                        1,
                        "within-budget",
                    );
                    observation.record_runtime_process_memory(memory);
                    record_observation(&tracer, observation, &live_store);
                }
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
                            record_observation(&tracer, observation, &live_store);
                        }
                    }
                }
            },
        }
    }
    while let Some(result) = ingress_connections.join_next().await {
        commit_ingress_completion(
            result,
            &tracer,
            latest_process_memory,
            &live_store,
            &dropped_observations,
        )
        .await;
    }
    memory_sampler.join().await??;
    let _ = query_shutdown.send(true);
    query_task.join().await??;
    let provider_shutdown = task_scope.spawn_blocking(
        "runtime-server-opentelemetry-provider-shutdown",
        move || provider.shutdown(),
    )?;
    task_scope.begin_drain();
    provider_shutdown
        .join()
        .await?
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
                let scheduler = super::process_memory::observe_runtime_scheduler();
                let observation = tokio::task::spawn_blocking(
                    move || {
                        super::process_memory::observe_process_memory(
                            event_loop_lag_micros,
                            scheduler,
                        )
                    },
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

async fn remove_socket_if_present(path: &std::path::Path) -> Result<(), String> {
    match tokio::fs::remove_file(path).await {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(format!(
            "failed to remove Runtime Server Telemetry socket {}: {error}",
            path.display()
        )),
    }
}

fn record_observation_with_memory(
    tracer: &opentelemetry_sdk::trace::SdkTracer,
    mut observation: RuntimePerformanceObservation,
    memory: Option<super::process_memory::ProcessMemoryObservation>,
    live_store: &super::live_store::RuntimePerformanceLiveStore,
) -> bool {
    if let Some(memory) = memory {
        observation.record_runtime_process_memory(memory);
    }
    record_observation(tracer, observation, live_store)
}

fn record_observation(
    tracer: &opentelemetry_sdk::trace::SdkTracer,
    mut observation: RuntimePerformanceObservation,
    live_store: &super::live_store::RuntimePerformanceLiveStore,
) -> bool {
    if observation.budget_status == "budget-exceeded" {
        observation.seal_budget_failure_identity();
    }
    if !live_store.record(&observation) {
        return false;
    }
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
    push_optional(
        &mut attributes,
        semconv::EVENT_IDENTITY,
        observation.event_identity,
    );
    push_optional(
        &mut attributes,
        semconv::INCIDENT_ID,
        observation.incident_id,
    );
    push_optional(
        &mut attributes,
        semconv::INCIDENT_STATE,
        observation.incident_state,
    );
    push_optional(
        &mut attributes,
        semconv::INCIDENT_TRANSITION,
        observation.incident_transition,
    );
    push_optional_u64(
        &mut attributes,
        semconv::INCIDENT_TRANSITION_SEQUENCE,
        observation.incident_transition_sequence,
    );
    push_optional(
        &mut attributes,
        semconv::REQUESTED_PROJECTION,
        observation.requested_projection,
    );
    push_optional_u64(
        &mut attributes,
        semconv::OBSERVED_AT_UNIX_MICROS,
        observation.observed_at_unix_micros,
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
        semconv::PROCESS_OPEN_DESCRIPTORS,
        observation.process_open_descriptors,
    );
    push_optional_u64(
        &mut attributes,
        semconv::PROCESS_OPEN_DESCRIPTOR_BUDGET,
        observation.process_open_descriptor_budget,
    );
    push_optional(
        &mut attributes,
        semconv::PROCESS_OPEN_DESCRIPTOR_BUDGET_STATUS,
        observation.process_open_descriptor_budget_status,
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
        semconv::RUNTIME_WORKER_THREADS,
        observation.runtime_worker_threads,
    );
    push_optional_u64(
        &mut attributes,
        semconv::RUNTIME_ALIVE_TASKS,
        observation.runtime_alive_tasks,
    );
    push_optional_u64(
        &mut attributes,
        semconv::RUNTIME_GLOBAL_QUEUE_DEPTH,
        observation.runtime_global_queue_depth,
    );
    push_optional_u64(
        &mut attributes,
        semconv::RUNTIME_ACTIVE_CONNECTIONS,
        observation.runtime_active_connections,
    );
    push_optional_u64(
        &mut attributes,
        semconv::RUNTIME_CONNECTION_LIMIT,
        observation.runtime_connection_limit,
    );
    push_optional_u64(
        &mut attributes,
        semconv::RUNTIME_CONNECTION_HIGH_WATERMARK,
        observation.runtime_connection_high_watermark,
    );
    push_optional_u64(
        &mut attributes,
        semconv::RUNTIME_REJECTED_CONNECTIONS,
        observation.runtime_rejected_connections,
    );
    push_optional_u64(
        &mut attributes,
        semconv::RUNTIME_DIAGNOSTIC_QUEUE_DEPTH,
        observation.runtime_diagnostic_queue_depth,
    );
    push_optional_u64(
        &mut attributes,
        semconv::RUNTIME_DIAGNOSTIC_QUEUE_CAPACITY,
        observation.runtime_diagnostic_queue_capacity,
    );
    push_optional_u64(
        &mut attributes,
        semconv::RUNTIME_DROPPED_DIAGNOSTICS,
        observation.runtime_dropped_diagnostics,
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
    push_optional_u64(
        &mut attributes,
        semconv::MEMORY_SEARCH_MAPPED_BYTES,
        observation.memory_search_mapped_bytes,
    );
    push_optional_u64(
        &mut attributes,
        semconv::MEMORY_SEARCH_DIRECTORY_BYTES_VALIDATED,
        observation.memory_search_directory_bytes_validated,
    );
    push_optional_u64(
        &mut attributes,
        semconv::MEMORY_SEARCH_KEY_BYTES_TOUCHED,
        observation.memory_search_key_bytes_touched,
    );
    push_optional_u64(
        &mut attributes,
        semconv::MEMORY_SEARCH_VALUE_BYTES_TOUCHED,
        observation.memory_search_value_bytes_touched,
    );
    push_optional_u64(
        &mut attributes,
        semconv::MEMORY_SEARCH_SOURCE_BYTES_READ,
        observation.memory_search_source_bytes_read,
    );
    push_optional_u64(
        &mut attributes,
        semconv::MEMORY_SEARCH_TURSO_OPENS,
        observation.memory_search_turso_opens,
    );
    push_optional_u64(
        &mut attributes,
        semconv::MEMORY_SEARCH_SOCKET_CONNECTS,
        observation.memory_search_socket_connects,
    );
    push_optional_u64(
        &mut attributes,
        semconv::MEMORY_SEARCH_PROVIDER_SPAWNS,
        observation.memory_search_provider_spawns,
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
    true
}

async fn read_ingress_observation(
    mut stream: tokio::net::UnixStream,
) -> Result<(RuntimePerformanceObservation, tokio::net::UnixStream), String> {
    let frame = read_ingress_frame(&mut stream).await?;
    let observation = serde_json::from_slice(&frame)
        .map_err(|error| format!("failed to decode Runtime Server telemetry ingress: {error}"))?;
    Ok((observation, stream))
}

async fn commit_ingress_completion(
    completed: Result<
        (
            crate::runtime_server_runtime::RuntimeServerConnectionLease,
            Result<(RuntimePerformanceObservation, tokio::net::UnixStream), String>,
        ),
        tokio::task::JoinError,
    >,
    tracer: &opentelemetry_sdk::trace::SdkTracer,
    memory: Option<super::process_memory::ProcessMemoryObservation>,
    live_store: &super::live_store::RuntimePerformanceLiveStore,
    dropped_observations: &std::sync::atomic::AtomicU64,
) {
    let Ok((_connection_lease, Ok((observation, mut stream)))) = completed else {
        dropped_observations.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        return;
    };
    let workspace_identity = observation.workspace_identity.clone().unwrap_or_default();
    let surface = observation.surface.clone();
    let stage = observation.stage.clone();
    let event_identity = observation.event_identity.clone();
    let recorded = record_observation_with_memory(tracer, observation, memory, live_store);
    let receipt = RuntimePerformanceIngressReceipt {
        schema_id: "agent.semantic-protocols.runtime-server-performance-ingress-receipt".to_owned(),
        schema_version: "1".to_owned(),
        state: if recorded { "recorded" } else { "duplicate" }.to_owned(),
        workspace_identity,
        surface,
        stage,
        event_identity,
    };
    let Ok(mut packet) = serde_json::to_vec(&receipt) else {
        dropped_observations.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        return;
    };
    packet.push(b'\n');
    if stream.write_all(&packet).await.is_err() {
        dropped_observations.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    }
}

async fn read_ingress_frame(stream: &mut tokio::net::UnixStream) -> Result<Vec<u8>, String> {
    const MAX_FRAME_BYTES: usize = 16 * 1024;
    let mut frame = Vec::with_capacity(1024);
    let mut buffer = [0_u8; 1024];
    loop {
        let length = stream
            .read(&mut buffer)
            .await
            .map_err(|error| format!("failed to read Runtime Server telemetry ingress: {error}"))?;
        if length == 0 {
            return Err("Runtime Server telemetry ingress closed before newline frame".to_owned());
        }
        let newline = buffer[..length].iter().position(|byte| *byte == b'\n');
        let take = newline.unwrap_or(length);
        if frame.len().saturating_add(take) > MAX_FRAME_BYTES {
            return Err("Runtime Server telemetry ingress frame exceeds 16 KiB".to_owned());
        }
        frame.extend_from_slice(&buffer[..take]);
        if newline.is_some() {
            return Ok(frame);
        }
    }
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
