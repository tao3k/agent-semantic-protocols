// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use tokio::sync::mpsc;

use crate::{
    runtime_server_opentelemetry::{RuntimeLifecycleEvent, RuntimePerformanceObservation},
    search_incident::{
        IncidentRecord, IncidentState, IncidentSurface, IncidentTelemetryEvent,
        SearchIncidentTerminalContext, SearchIncidentTerminalOutcome, TransitionError,
        observe_terminal,
    },
};

pub const CAPACITY: usize = 1024;

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResidentReadTerminalContext {
    pub operation_id: String,
    pub surface: String,
    pub workspace_identity: String,
    pub generation_digest: String,
    pub root_digest: String,
    pub read_state: crate::workspace_db_ipc::RuntimeResidentReadState,
    pub elapsed_micros: u64,
    pub work_counters: crate::workspace_db_ipc::WorkspaceIpcResidentReadWorkCounters,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResidentReadTerminalOutcome {
    pub terminal_state: crate::workspace_db_ipc::RuntimeResidentReadTerminalState,
}

#[derive(Clone)]
pub struct RuntimeTelemetryBusSender {
    terminal: mpsc::Sender<RuntimeTelemetryEnvelope>,
    ordered: mpsc::Sender<RuntimeTelemetryEnvelope>,
    transition_permits: std::sync::Arc<tokio::sync::Semaphore>,
    pending_transitions: std::sync::Arc<dashmap::DashMap<String, usize>>,
    incidents: std::sync::Arc<dashmap::DashMap<(String, String), IncidentRecord>>,
    resident_reads: std::sync::Arc<dashmap::DashMap<(String, String), ResidentReadTerminalOutcome>>,
}

#[derive(Debug, Eq, PartialEq)]
pub enum RuntimeIncidentAdmissionError {
    InvalidTransition(TransitionError),
    TerminalQueueFull {
        workspace_identity: String,
        incident_id: String,
    },
}

pub struct RuntimeTelemetryBusReceiver {
    terminal: mpsc::Receiver<RuntimeTelemetryEnvelope>,
    ordered: mpsc::Receiver<RuntimeTelemetryEnvelope>,
    pending_transitions: std::sync::Arc<dashmap::DashMap<String, usize>>,
}

struct RuntimeTelemetryEnvelope {
    event: RuntimeTelemetryEvent,
    _transition_permit: Option<tokio::sync::OwnedSemaphorePermit>,
    pending_transition_key: Option<String>,
}

pub enum RuntimeTelemetryEvent {
    Lifecycle(RuntimeLifecycleEvent),
    SearchIncident(IncidentTelemetryEvent),
    Performance(RuntimePerformanceObservation),
}

pub struct RuntimeTelemetryBus {
    pub sender: RuntimeTelemetryBusSender,
    pub receiver: RuntimeTelemetryBusReceiver,
}

impl RuntimeTelemetryBus {
    pub fn new() -> Self {
        let (terminal, terminal_receiver) = mpsc::channel(CAPACITY);
        let (ordered, ordered_receiver) = mpsc::channel(CAPACITY * 2);
        let pending_transitions = std::sync::Arc::new(dashmap::DashMap::new());
        let resident_reads = std::sync::Arc::new(dashmap::DashMap::new());
        Self {
            sender: RuntimeTelemetryBusSender {
                terminal,
                ordered,
                transition_permits: std::sync::Arc::new(tokio::sync::Semaphore::new(CAPACITY)),
                pending_transitions: std::sync::Arc::clone(&pending_transitions),
                incidents: std::sync::Arc::new(dashmap::DashMap::new()),
                resident_reads: std::sync::Arc::clone(&resident_reads),
            },
            receiver: RuntimeTelemetryBusReceiver {
                terminal: terminal_receiver,
                ordered: ordered_receiver,
                pending_transitions,
            },
        }
    }
}

impl RuntimeTelemetryBusSender {
    pub fn try_record_resident_read_terminal(
        &self,
        context: ResidentReadTerminalContext,
        outcome: ResidentReadTerminalOutcome,
    ) -> Result<String, String> {
        if context.operation_id.trim().is_empty() || context.surface.trim().is_empty() {
            return Err("resident read telemetry requires operationId and surface".to_owned());
        }
        let digest = crate::workspace_db_ipc::resident_read_terminal_digest(
            &crate::workspace_db_ipc::RuntimeResidentReadTerminalDigestInput {
                operation_id: &context.operation_id,
                surface: &context.surface,
                workspace_identity: &context.workspace_identity,
                generation_digest: &context.generation_digest,
                root_digest: &context.root_digest,
                read_state: context.read_state,
                elapsed_micros: context.elapsed_micros,
                terminal_state: outcome.terminal_state,
            },
        );
        self.resident_reads.insert(
            (context.operation_id.clone(), context.surface.clone()),
            outcome,
        );
        if let Some(observation) = context.into_performance_observation() {
            self.try_record_performance(observation)?;
        }
        Ok(digest)
    }

    pub fn try_record_performance(
        &self,
        observation: RuntimePerformanceObservation,
    ) -> Result<(), String> {
        let event = RuntimeTelemetryEvent::Performance(observation);
        let sender = if self.has_pending_transition(&telemetry_order_key(&event)) {
            &self.ordered
        } else {
            &self.terminal
        };
        sender
            .try_send(RuntimeTelemetryEnvelope {
                event,
                _transition_permit: None,
                pending_transition_key: None,
            })
            .map_err(|_| "Runtime performance telemetry queue is full or closed".to_owned())
    }

    pub fn resident_read_terminal_exists(&self, operation_id: &str, surface: &str) -> bool {
        self.resident_reads
            .contains_key(&(operation_id.to_owned(), surface.to_owned()))
    }

    pub fn try_record_search_terminal(
        &self,
        context: SearchIncidentTerminalContext,
        outcome: SearchIncidentTerminalOutcome,
    ) -> Result<Option<IncidentRecord>, RuntimeIncidentAdmissionError> {
        let Some((initial_record, initial_event)) =
            observe_terminal(None, context.clone(), outcome.clone())
                .map_err(RuntimeIncidentAdmissionError::InvalidTransition)?
        else {
            return Ok(None);
        };
        let key = (
            initial_record.identity.workspace_identity.clone(),
            initial_record.identity.incident_id.clone(),
        );
        match self.incidents.entry(key) {
            dashmap::mapref::entry::Entry::Occupied(mut entry) => {
                let Some((record, event)) =
                    observe_terminal(Some(entry.get().clone()), context, outcome)
                        .map_err(RuntimeIncidentAdmissionError::InvalidTransition)?
                else {
                    return Ok(None);
                };
                self.try_record_incident_terminal(event).map_err(|event| {
                    RuntimeIncidentAdmissionError::TerminalQueueFull {
                        workspace_identity: event.observation.identity.workspace_identity,
                        incident_id: event.observation.identity.incident_id,
                    }
                })?;
                entry.insert(record.clone());
                Ok(Some(record))
            }
            dashmap::mapref::entry::Entry::Vacant(entry) => {
                self.try_record_incident_terminal(initial_event)
                    .map_err(|event| RuntimeIncidentAdmissionError::TerminalQueueFull {
                        workspace_identity: event.observation.identity.workspace_identity,
                        incident_id: event.observation.identity.incident_id,
                    })?;
                entry.insert(initial_record.clone());
                Ok(Some(initial_record))
            }
        }
    }

    pub async fn send_terminal(
        &self,
        event: RuntimeLifecycleEvent,
    ) -> Result<(), RuntimeLifecycleEvent> {
        let event = RuntimeTelemetryEvent::Lifecycle(event);
        let sender = if self.has_pending_transition(&telemetry_order_key(&event)) {
            &self.ordered
        } else {
            &self.terminal
        };
        sender
            .send(RuntimeTelemetryEnvelope {
                event,
                _transition_permit: None,
                pending_transition_key: None,
            })
            .await
            .map_err(|error| match error.0.event {
                RuntimeTelemetryEvent::Lifecycle(event) => event,
                RuntimeTelemetryEvent::SearchIncident(_) => {
                    unreachable!("lifecycle send returned a different telemetry variant")
                }
                RuntimeTelemetryEvent::Performance(_) => {
                    unreachable!("lifecycle send returned a different telemetry variant")
                }
            })
    }

    pub fn try_send_transition(&self, event: RuntimeLifecycleEvent) -> bool {
        let Ok(permit) = std::sync::Arc::clone(&self.transition_permits).try_acquire_owned() else {
            return false;
        };
        let event = RuntimeTelemetryEvent::Lifecycle(event);
        let key = telemetry_order_key(&event);
        self.increment_pending_transition(&key);
        let sent = self
            .ordered
            .try_send(RuntimeTelemetryEnvelope {
                event,
                _transition_permit: Some(permit),
                pending_transition_key: Some(key.clone()),
            })
            .is_ok();
        if !sent {
            decrement_pending_transition(&self.pending_transitions, &key);
        }
        sent
    }

    pub fn try_record_incident_terminal(
        &self,
        event: IncidentTelemetryEvent,
    ) -> Result<(), IncidentTelemetryEvent> {
        if !event.is_valid() {
            return Err(event);
        }
        let event = RuntimeTelemetryEvent::SearchIncident(event);
        let sender = if self.has_pending_transition(&telemetry_order_key(&event)) {
            &self.ordered
        } else {
            &self.terminal
        };
        sender
            .try_send(RuntimeTelemetryEnvelope {
                event,
                _transition_permit: None,
                pending_transition_key: None,
            })
            .map_err(|error| match error.into_inner().event {
                RuntimeTelemetryEvent::SearchIncident(event) => event,
                RuntimeTelemetryEvent::Lifecycle(_) => {
                    unreachable!("incident send returned a different telemetry variant")
                }
                RuntimeTelemetryEvent::Performance(_) => {
                    unreachable!("incident send returned a different telemetry variant")
                }
            })
    }

    pub fn try_send_incident_transition(&self, event: IncidentTelemetryEvent) -> bool {
        if !event.is_valid() {
            return false;
        }
        let Ok(permit) = std::sync::Arc::clone(&self.transition_permits).try_acquire_owned() else {
            return false;
        };
        let event = RuntimeTelemetryEvent::SearchIncident(event);
        let key = telemetry_order_key(&event);
        self.increment_pending_transition(&key);
        let sent = self
            .ordered
            .try_send(RuntimeTelemetryEnvelope {
                event,
                _transition_permit: Some(permit),
                pending_transition_key: Some(key.clone()),
            })
            .is_ok();
        if !sent {
            decrement_pending_transition(&self.pending_transitions, &key);
        }
        sent
    }

    fn has_pending_transition(&self, key: &str) -> bool {
        self.pending_transitions
            .get(key)
            .is_some_and(|count| *count > 0)
    }

    fn increment_pending_transition(&self, key: &str) {
        self.pending_transitions
            .entry(key.to_owned())
            .and_modify(|count| *count += 1)
            .or_insert(1);
    }
}

impl ResidentReadTerminalContext {
    pub fn into_performance_observation(self) -> Option<RuntimePerformanceObservation> {
        if self.surface != "runtime-resident-runtime-selector" {
            return None;
        }
        let budget_micros = 1_000;
        let budget_status = if self.elapsed_micros <= budget_micros {
            "within-budget"
        } else {
            "budget-exceeded"
        };
        let mut observation = RuntimePerformanceObservation::new(
            "query",
            "runtime-selector-read",
            self.elapsed_micros,
            budget_micros,
            budget_status,
        );
        observation.workspace_identity = Some(self.workspace_identity);
        observation.generation_digest = Some(self.generation_digest);
        observation.operation_id = Some(self.operation_id);
        observation.requested_projection = Some("exact-selector".to_owned());
        observation.memory_search_turso_opens = Some(self.work_counters.database_opens);
        observation.memory_search_source_bytes_read = Some(self.work_counters.filesystem_reads);
        observation.memory_search_provider_spawns = Some(self.work_counters.provider_spawns);
        observation.memory_search_socket_connects =
            Some(self.work_counters.control_socket_roundtrips);
        Some(observation)
    }
}

impl RuntimeTelemetryBusReceiver {
    pub fn close(&mut self) {
        self.terminal.close();
        self.ordered.close();
    }

    pub async fn recv(&mut self) -> Option<RuntimeTelemetryEvent> {
        let envelope = tokio::select! {
            biased;
            event = self.terminal.recv() => match event {
                Some(event) => Some(event),
                None => self.ordered.recv().await,
            },
            event = self.ordered.recv() => match event {
                Some(event) => Some(event),
                None => self.terminal.recv().await,
            },
        }?;
        Some(self.complete(envelope))
    }

    pub fn try_recv(
        &mut self,
    ) -> Result<RuntimeTelemetryEvent, tokio::sync::mpsc::error::TryRecvError> {
        match self.terminal.try_recv() {
            Ok(envelope) => Ok(self.complete(envelope)),
            Err(tokio::sync::mpsc::error::TryRecvError::Disconnected)
            | Err(tokio::sync::mpsc::error::TryRecvError::Empty) => self
                .ordered
                .try_recv()
                .map(|envelope| self.complete(envelope)),
        }
    }

    fn complete(&self, envelope: RuntimeTelemetryEnvelope) -> RuntimeTelemetryEvent {
        if let Some(key) = envelope.pending_transition_key.as_deref() {
            decrement_pending_transition(&self.pending_transitions, key);
        }
        envelope.event
    }
}

fn telemetry_order_key(event: &RuntimeTelemetryEvent) -> String {
    match event {
        RuntimeTelemetryEvent::Lifecycle(event) => format!(
            "lifecycle:{}:{}",
            event.workspace_identity.as_deref().unwrap_or_default(),
            event.owner_epoch
        ),
        RuntimeTelemetryEvent::SearchIncident(event) => format!(
            "incident:{}:{}",
            event.observation.identity.workspace_identity, event.observation.identity.incident_id
        ),
        RuntimeTelemetryEvent::Performance(event) => format!(
            "performance:{}:{}",
            event.workspace_identity.as_deref().unwrap_or_default(),
            event.operation_id.as_deref().unwrap_or_default()
        ),
    }
}

fn decrement_pending_transition(pending: &dashmap::DashMap<String, usize>, key: &str) {
    if let dashmap::mapref::entry::Entry::Occupied(mut entry) = pending.entry(key.to_owned()) {
        if *entry.get() == 1 {
            entry.remove();
        } else {
            *entry.get_mut() -= 1;
        }
    }
}

impl RuntimeTelemetryEvent {
    pub fn into_observation(self) -> RuntimePerformanceObservation {
        match self {
            Self::Lifecycle(event) => event.into_observation(),
            Self::SearchIncident(event) => incident_observation(event),
            Self::Performance(event) => event,
        }
    }
}

fn incident_observation(event: IncidentTelemetryEvent) -> RuntimePerformanceObservation {
    let incident = event.observation;
    let surface = match incident.identity.surface {
        IncidentSurface::Search => "search",
        IncidentSurface::Query => "query",
    };
    let budget_status = if incident.budget_exceeded {
        "budget-exceeded"
    } else {
        match event.state {
            IncidentState::Resolved | IncidentState::Superseded | IncidentState::Compacted => "ok",
            IncidentState::FailedVerification => "failed-verification",
            IncidentState::Open | IncidentState::Repairing | IncidentState::VerificationPending => {
                "incident"
            }
        }
    };
    let mut observation = RuntimePerformanceObservation::new(
        surface,
        incident.identity.stage.clone(),
        incident.elapsed_micros.unwrap_or_default(),
        incident.budget_micros.unwrap_or_default(),
        budget_status,
    );
    observation.workspace_identity = Some(incident.identity.workspace_identity);
    observation.language_id = Some(incident.identity.language_id);
    observation.generation_digest = incident.identity.generation_digest;
    observation.runtime_artifact_digest = incident.identity.runtime_artifact_digest;
    observation.operation_id = Some(incident.identity.canonical_request_digest);
    observation.event_identity = Some(format!(
        "{}:{}:{}:{}",
        observation
            .workspace_identity
            .as_deref()
            .unwrap_or_default(),
        incident.identity.incident_id,
        event.transition,
        event.transition_sequence
    ));
    observation.incident_id = Some(incident.identity.incident_id);
    observation.incident_state = Some(
        match event.state {
            IncidentState::Open => "open",
            IncidentState::Repairing => "repairing",
            IncidentState::VerificationPending => "verification-pending",
            IncidentState::FailedVerification => "failed-verification",
            IncidentState::Resolved => "resolved",
            IncidentState::Superseded => "superseded",
            IncidentState::Compacted => "compacted",
        }
        .to_owned(),
    );
    observation.incident_transition = Some(event.transition);
    observation.incident_transition_sequence = Some(event.transition_sequence);
    observation.requested_projection = Some(
        match incident.identity.requested_projection {
            crate::search_incident::RequestedProjection::None => "none",
            crate::search_incident::RequestedProjection::Seeds => "seeds",
            crate::search_incident::RequestedProjection::Source => "source",
            crate::search_incident::RequestedProjection::CallableSkeleton => "callable-skeleton",
            crate::search_incident::RequestedProjection::Packet => "packet",
        }
        .to_owned(),
    );
    observation.failure_reason = Some(incident.identity.reason_kind);
    observation.observed_at_unix_micros = Some(incident.observed_at_unix_micros);
    observation.process_resident_bytes = incident.resources.resident_memory_bytes;
    observation.process_peak_resident_bytes = incident.resources.peak_resident_memory_bytes;
    observation.process_disk_read_bytes = incident.resources.disk_read_bytes;
    observation.process_disk_write_bytes = incident.resources.disk_write_bytes;
    observation.runtime_event_loop_lag_micros = incident.resources.event_loop_lag_micros;
    observation.runtime_alive_tasks = incident.resources.active_task_count;
    observation.runtime_active_connections = incident.resources.active_child_count;
    observation.runtime_global_queue_depth = incident.resources.writer_queue_depth;
    observation
}
