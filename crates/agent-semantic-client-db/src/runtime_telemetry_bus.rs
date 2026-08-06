use tokio::sync::mpsc;

use crate::{
    runtime_server_opentelemetry::{RuntimeLifecycleEvent, RuntimePerformanceObservation},
    search_incident::{IncidentState, IncidentSurface, IncidentTelemetryEvent},
};

pub const CAPACITY: usize = 1024;

#[derive(Clone)]
pub struct RuntimeTelemetryBusSender {
    terminal: mpsc::Sender<RuntimeTelemetryEvent>,
    transition: mpsc::Sender<RuntimeTelemetryEvent>,
}

pub struct RuntimeTelemetryBusReceiver {
    terminal: mpsc::Receiver<RuntimeTelemetryEvent>,
    transition: mpsc::Receiver<RuntimeTelemetryEvent>,
}

pub enum RuntimeTelemetryEvent {
    Lifecycle(RuntimeLifecycleEvent),
    SearchIncident(IncidentTelemetryEvent),
}

pub struct RuntimeTelemetryBus {
    pub sender: RuntimeTelemetryBusSender,
    pub receiver: RuntimeTelemetryBusReceiver,
}

impl RuntimeTelemetryBus {
    pub fn new() -> Self {
        let (terminal, terminal_receiver) = mpsc::channel(CAPACITY);
        let (transition, transition_receiver) = mpsc::channel(CAPACITY);
        Self {
            sender: RuntimeTelemetryBusSender {
                terminal,
                transition,
            },
            receiver: RuntimeTelemetryBusReceiver {
                terminal: terminal_receiver,
                transition: transition_receiver,
            },
        }
    }
}

impl RuntimeTelemetryBusSender {
    pub async fn send_terminal(
        &self,
        event: RuntimeLifecycleEvent,
    ) -> Result<(), RuntimeLifecycleEvent> {
        self.terminal
            .send(RuntimeTelemetryEvent::Lifecycle(event))
            .await
            .map_err(|error| match error.0 {
                RuntimeTelemetryEvent::Lifecycle(event) => event,
                RuntimeTelemetryEvent::SearchIncident(_) => {
                    unreachable!("lifecycle send returned a different telemetry variant")
                }
            })
    }

    pub fn try_send_transition(&self, event: RuntimeLifecycleEvent) -> bool {
        self.transition
            .try_send(RuntimeTelemetryEvent::Lifecycle(event))
            .is_ok()
    }

    pub fn try_record_incident_terminal(
        &self,
        event: IncidentTelemetryEvent,
    ) -> Result<(), IncidentTelemetryEvent> {
        if !event.is_valid() {
            return Err(event);
        }
        self.terminal
            .try_send(RuntimeTelemetryEvent::SearchIncident(event))
            .map_err(|error| match error.0 {
                RuntimeTelemetryEvent::SearchIncident(event) => event,
                RuntimeTelemetryEvent::Lifecycle(_) => {
                    unreachable!("incident send returned a different telemetry variant")
                }
            })
    }

    pub fn try_send_incident_transition(&self, event: IncidentTelemetryEvent) -> bool {
        if !event.is_valid() {
            return false;
        }
        self.transition
            .try_send(RuntimeTelemetryEvent::SearchIncident(event))
            .is_ok()
    }
}

impl RuntimeTelemetryBusReceiver {
    pub fn close(&mut self) {
        self.terminal.close();
        self.transition.close();
    }

    pub async fn recv(&mut self) -> Option<RuntimeTelemetryEvent> {
        tokio::select! {
            biased;
            event = self.terminal.recv() => match event {
                Some(event) => Some(event),
                None => self.transition.recv().await,
            },
            event = self.transition.recv() => match event {
                Some(event) => Some(event),
                None => self.terminal.recv().await,
            },
        }
    }

    pub fn try_recv(
        &mut self,
    ) -> Result<RuntimeTelemetryEvent, tokio::sync::mpsc::error::TryRecvError> {
        match self.terminal.try_recv() {
            Ok(event) => Ok(event),
            Err(tokio::sync::mpsc::error::TryRecvError::Disconnected) => self.transition.try_recv(),
            Err(tokio::sync::mpsc::error::TryRecvError::Empty) => self.transition.try_recv(),
        }
    }
}

impl RuntimeTelemetryEvent {
    pub fn into_observation(self) -> RuntimePerformanceObservation {
        match self {
            Self::Lifecycle(event) => event.into_observation(),
            Self::SearchIncident(event) => incident_observation(event),
        }
    }
}

fn incident_observation(event: IncidentTelemetryEvent) -> RuntimePerformanceObservation {
    let incident = event.observation;
    let surface = match incident.identity.surface {
        IncidentSurface::Search => "search",
        IncidentSurface::Query => "query",
    };
    let budget_status = match event.state {
        IncidentState::Resolved | IncidentState::Superseded | IncidentState::Compacted => "ok",
        IncidentState::FailedVerification => "failed-verification",
        IncidentState::Open | IncidentState::Repairing | IncidentState::VerificationPending => {
            "incident"
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
