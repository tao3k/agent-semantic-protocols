use std::{path::Path, sync::Arc};

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
    /// Starts immediately. Turso open and schema bootstrap occur only inside
    /// the resident task and cannot delay core endpoint publication.
    pub fn start(
        database_path: std::path::PathBuf,
        ingress_socket_path: std::path::PathBuf,
        query_socket_path: std::path::PathBuf,
    ) -> Result<Self, String> {
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

/// Emits one observation to the resident lane without awaiting I/O. This is
/// used only after a performance gate has already failed; success and warm
/// paths allocate no telemetry socket.
pub fn try_emit_to_runtime(
    ingress_socket_path: &Path,
    observation: &RuntimePerformanceObservation,
) -> bool {
    let Ok(bytes) = serde_json::to_vec(observation) else {
        return false;
    };
    let Ok(socket) = std::os::unix::net::UnixDatagram::unbound() else {
        return false;
    };
    if socket.set_nonblocking(true).is_err() {
        return false;
    }
    socket.send_to(&bytes, ingress_socket_path).is_ok()
}

async fn run_resident_telemetry_lane(
    database_path: std::path::PathBuf,
    mut receiver: mpsc::Receiver<RuntimePerformanceObservation>,
    ingress: tokio::net::UnixDatagram,
    query_listener: tokio::net::UnixListener,
    dropped_observations: Arc<std::sync::atomic::AtomicU64>,
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
    loop {
        tokio::select! {
            observation = receiver.recv() => match observation {
                Some(observation) => record_observation(&tracer, observation),
                None => break,
            },
            ingress_result = ingress.recv(&mut ingress_buffer) => match ingress_result {
                Ok(length) => match serde_json::from_slice::<RuntimePerformanceObservation>(
                    &ingress_buffer[..length],
                ) {
                    Ok(observation) => record_observation(&tracer, observation),
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
            changed = shutdown.changed() => {
                let _ = changed;
                while let Ok(observation) = receiver.try_recv() {
                    record_observation(&tracer, observation);
                }
                break;
            }
        }
    }
    let _ = query_shutdown.send(true);
    query_task
        .await
        .map_err(|error| format!("Runtime Server telemetry query server failed: {error}"))??;
    tokio::task::spawn_blocking(move || provider.shutdown())
        .await
        .map_err(|error| format!("OpenTelemetry provider shutdown task failed: {error}"))?
        .map_err(|error| format!("OpenTelemetry provider shutdown failed: {error}"))
}

fn record_observation(
    tracer: &opentelemetry_sdk::trace::SdkTracer,
    observation: RuntimePerformanceObservation,
) {
    let mut span = tracer.start(format!("asp {} {}", observation.surface, observation.stage));
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
