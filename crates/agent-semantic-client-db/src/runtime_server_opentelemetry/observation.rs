// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

pub const RUNTIME_SEARCH_TELEMETRY_PHASES: [&str; 10] = [
    "launcher",
    "client-frame-encode",
    "ipc-connect",
    "server-admission-queue",
    "snapshot-resolve",
    "provider-dispatch",
    "parse-index-query",
    "projection-rank",
    "schema-validate-serialize",
    "terminal-egress",
];

pub const RUNTIME_SEARCH_TELEMETRY_SCHEMA_REF: &str =
    "runtime-server-opentelemetry-performance-event";

#[derive(Clone, Debug, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeSearchTelemetryIdentity {
    pub schema_ref: String,
    pub session_id: String,
    pub request_id: String,
    pub workspace_identity: String,
    pub workspace_snapshot_digest: String,
    pub source_snapshot_digest: String,
    pub source_generation_digest: String,
    pub source_index_digest: String,
    pub runtime_artifact_digest: String,
    pub runtime_bundle_digest: String,
    pub execution_publication_digest: String,
    pub provider_catalog_digest: String,
    pub language_ids: Vec<String>,
    pub provider_ids: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RuntimeSearchTelemetryIdentityInput {
    pub session_id: String,
    pub request_id: String,
    pub workspace_identity: String,
    pub workspace_snapshot_digest: String,
    pub source_snapshot_digest: String,
    pub source_generation_digest: String,
    pub source_index_digest: String,
    pub runtime_artifact_digest: String,
    pub runtime_bundle_digest: String,
    pub execution_publication_digest: String,
    pub provider_catalog_digest: String,
    pub language_ids: Vec<String>,
    pub provider_ids: Vec<String>,
}

impl RuntimeSearchTelemetryIdentity {
    pub fn settle_client_timing(
        publication: &agent_semantic_content_identity::runtime_workspace_execution_publication::RuntimeWorkspaceExecutionPublication,
        witness: &agent_semantic_client_protocol::RuntimeSearchClientTimingWitness,
        session_id: &str,
        request_id: &str,
        language_ids: Vec<String>,
        provider_ids: Vec<String>,
    ) -> Result<Self, RuntimeSearchTelemetryError> {
        witness
            .admit_for_request(session_id, request_id)
            .map_err(|_| {
                RuntimeSearchTelemetryError::new("runtime-search-client-timing-identity-mismatch")
            })?;
        Self::from_execution_publication(
            publication,
            session_id,
            request_id,
            language_ids,
            provider_ids,
        )
    }

    pub fn from_execution_publication(
        publication: &agent_semantic_content_identity::runtime_workspace_execution_publication::RuntimeWorkspaceExecutionPublication,
        session_id: impl Into<String>,
        request_id: impl Into<String>,
        language_ids: Vec<String>,
        provider_ids: Vec<String>,
    ) -> Result<Self, RuntimeSearchTelemetryError> {
        publication.validate().map_err(|_| {
            RuntimeSearchTelemetryError::new("runtime-search-telemetry-identity-mismatch")
        })?;
        let content = &publication
            .runtime_execution_binding
            .content_binding
            .identity;
        Self::new(RuntimeSearchTelemetryIdentityInput {
            session_id: session_id.into(),
            request_id: request_id.into(),
            workspace_identity: publication.workspace_identity.clone(),
            workspace_snapshot_digest: content.workspace_snapshot_digest.clone(),
            source_snapshot_digest: publication.source_root_digest.as_str().to_owned(),
            source_generation_digest: content.source_generation_digest.clone(),
            source_index_digest: content.source_index_digest.clone(),
            runtime_artifact_digest: content.runtime_artifact_digest.clone(),
            runtime_bundle_digest: publication.runtime_bundle_digest.as_str().to_owned(),
            execution_publication_digest: publication.publication_digest.as_str().to_owned(),
            provider_catalog_digest: content.provider_catalog_digest.clone(),
            language_ids,
            provider_ids,
        })
    }

    pub fn new(
        input: RuntimeSearchTelemetryIdentityInput,
    ) -> Result<Self, RuntimeSearchTelemetryError> {
        let identity = Self {
            schema_ref: RUNTIME_SEARCH_TELEMETRY_SCHEMA_REF.into(),
            session_id: input.session_id,
            request_id: input.request_id,
            workspace_identity: input.workspace_identity,
            workspace_snapshot_digest: input.workspace_snapshot_digest,
            source_snapshot_digest: input.source_snapshot_digest,
            source_generation_digest: input.source_generation_digest,
            source_index_digest: input.source_index_digest,
            runtime_artifact_digest: input.runtime_artifact_digest,
            runtime_bundle_digest: input.runtime_bundle_digest,
            execution_publication_digest: input.execution_publication_digest,
            provider_catalog_digest: input.provider_catalog_digest,
            language_ids: input.language_ids,
            provider_ids: input.provider_ids,
        };
        if [
            identity.session_id.as_str(),
            identity.request_id.as_str(),
            identity.workspace_identity.as_str(),
        ]
        .iter()
        .any(|value| value.is_empty())
            || !runtime_search_identity_set_is_canonical(&identity.language_ids)
            || !runtime_search_identity_set_is_canonical(&identity.provider_ids)
            || [
                identity.workspace_snapshot_digest.as_str(),
                identity.source_snapshot_digest.as_str(),
                identity.source_generation_digest.as_str(),
                identity.source_index_digest.as_str(),
                identity.runtime_artifact_digest.as_str(),
                identity.runtime_bundle_digest.as_str(),
                identity.execution_publication_digest.as_str(),
                identity.provider_catalog_digest.as_str(),
            ]
            .iter()
            .any(|value| !runtime_search_content_digest(value))
        {
            return Err(RuntimeSearchTelemetryError::new(
                "runtime-search-telemetry-identity-mismatch",
            ));
        }
        Ok(identity)
    }
}

fn runtime_search_identity_set_is_canonical(values: &[String]) -> bool {
    !values.is_empty()
        && values.iter().all(|value| !value.trim().is_empty())
        && values.windows(2).all(|pair| pair[0] < pair[1])
}

fn runtime_search_content_digest(value: &str) -> bool {
    value.strip_prefix("blake3-256:").is_some_and(|hex| {
        hex.len() == 64
            && hex
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
    })
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RuntimeSearchTelemetryError {
    reason_kind: &'static str,
}

impl RuntimeSearchTelemetryError {
    fn new(reason_kind: &'static str) -> Self {
        Self { reason_kind }
    }

    pub fn reason_kind(&self) -> &str {
        self.reason_kind
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RuntimeSearchTelemetryArtifact {
    phase_names: [&'static str; 10],
    terminal_count: usize,
}

impl RuntimeSearchTelemetryArtifact {
    pub fn phase_names(&self) -> [&'static str; 10] {
        self.phase_names
    }

    pub fn terminal_count(&self) -> usize {
        self.terminal_count
    }
}

pub struct RuntimeSearchTelemetryCollector {
    identity: RuntimeSearchTelemetryIdentity,
    capacity: usize,
    phase_index: usize,
    terminal_count: usize,
}

impl RuntimeSearchTelemetryCollector {
    pub fn new(
        identity: RuntimeSearchTelemetryIdentity,
        capacity: usize,
    ) -> Result<Self, RuntimeSearchTelemetryError> {
        if capacity == 0 {
            return Err(RuntimeSearchTelemetryError::new(
                "runtime-search-telemetry-capacity-exhausted",
            ));
        }
        Ok(Self {
            identity,
            capacity,
            phase_index: 0,
            terminal_count: 0,
        })
    }

    fn validate_identity(
        &self,
        identity: &RuntimeSearchTelemetryIdentity,
    ) -> Result<(), RuntimeSearchTelemetryError> {
        if identity != &self.identity {
            return Err(RuntimeSearchTelemetryError::new(
                "runtime-search-telemetry-identity-mismatch",
            ));
        }
        Ok(())
    }

    pub fn record_phase(
        &mut self,
        identity: &RuntimeSearchTelemetryIdentity,
        phase: &str,
        _elapsed_micros: u64,
    ) -> Result<(), RuntimeSearchTelemetryError> {
        self.validate_identity(identity)?;
        if self.phase_index >= RUNTIME_SEARCH_TELEMETRY_PHASES.len()
            || RUNTIME_SEARCH_TELEMETRY_PHASES[self.phase_index] != phase
        {
            return Err(RuntimeSearchTelemetryError::new(
                "runtime-search-telemetry-phase-order",
            ));
        }
        if self.phase_index + 1 > self.capacity.saturating_sub(1) {
            return Err(RuntimeSearchTelemetryError::new(
                "runtime-search-telemetry-capacity-exhausted",
            ));
        }
        self.phase_index += 1;
        Ok(())
    }

    pub fn record_terminal(
        &mut self,
        identity: &RuntimeSearchTelemetryIdentity,
        _state: &str,
        _elapsed_micros: u64,
    ) -> Result<(), RuntimeSearchTelemetryError> {
        self.validate_identity(identity)?;
        if self.terminal_count != 0 {
            return Err(RuntimeSearchTelemetryError::new(
                "runtime-search-terminal-duplicate",
            ));
        }
        self.terminal_count = 1;
        Ok(())
    }

    pub fn record_metric_label(
        &mut self,
        key: &str,
        value: &str,
    ) -> Result<(), RuntimeSearchTelemetryError> {
        if !matches!(
            key,
            "language_id" | "operation" | "profile" | "phase" | "outcome" | "error_class"
        ) {
            return Err(RuntimeSearchTelemetryError::new(
                "runtime-search-metric-label-cardinality",
            ));
        }
        if value.is_empty() {
            return Err(RuntimeSearchTelemetryError::new(
                "runtime-search-metric-label-cardinality",
            ));
        }
        Ok(())
    }

    pub fn to_artifact(
        &self,
    ) -> Result<RuntimeSearchTelemetryArtifact, RuntimeSearchTelemetryError> {
        if self.phase_index != RUNTIME_SEARCH_TELEMETRY_PHASES.len() || self.terminal_count != 1 {
            return Err(RuntimeSearchTelemetryError::new(
                "runtime-search-telemetry-terminal-incomplete",
            ));
        }
        Ok(RuntimeSearchTelemetryArtifact {
            phase_names: RUNTIME_SEARCH_TELEMETRY_PHASES,
            terminal_count: self.terminal_count,
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum RuntimeSearchTelemetryPhaseOwner {
    Client,
    Admission,
    RuntimeRoute,
    SearchExecution,
    SearchProjection,
    ResponseService,
    TransportWriter,
}

fn runtime_search_telemetry_phase_owner(phase: &str) -> Option<RuntimeSearchTelemetryPhaseOwner> {
    match phase {
        "launcher" | "client-frame-encode" | "ipc-connect" => {
            Some(RuntimeSearchTelemetryPhaseOwner::Client)
        }
        "server-admission-queue" => Some(RuntimeSearchTelemetryPhaseOwner::Admission),
        "snapshot-resolve" | "provider-dispatch" => {
            Some(RuntimeSearchTelemetryPhaseOwner::RuntimeRoute)
        }
        "parse-index-query" => Some(RuntimeSearchTelemetryPhaseOwner::SearchExecution),
        "projection-rank" => Some(RuntimeSearchTelemetryPhaseOwner::SearchProjection),
        "schema-validate-serialize" => Some(RuntimeSearchTelemetryPhaseOwner::ResponseService),
        "terminal-egress" => Some(RuntimeSearchTelemetryPhaseOwner::TransportWriter),
        _ => None,
    }
}

#[derive(Debug, Default)]
struct RuntimeSearchTelemetryTraceState {
    observations: Vec<RuntimePerformanceObservation>,
    next_phase: usize,
    terminal_count: usize,
}

/// One bounded request-scoped trace. Its mutex protects only ten in-memory
/// records and is never held across an await, provider call, or filesystem I/O.
#[derive(Clone, Debug)]
pub struct RuntimeSearchTelemetryTrace {
    identity: RuntimeSearchTelemetryIdentity,
    state: std::sync::Arc<std::sync::Mutex<RuntimeSearchTelemetryTraceState>>,
}

impl RuntimeSearchTelemetryTrace {
    pub fn new(identity: RuntimeSearchTelemetryIdentity) -> Self {
        Self {
            identity,
            state: std::sync::Arc::new(std::sync::Mutex::new(RuntimeSearchTelemetryTraceState {
                observations: Vec::with_capacity(RUNTIME_SEARCH_TELEMETRY_PHASES.len()),
                ..RuntimeSearchTelemetryTraceState::default()
            })),
        }
    }

    pub fn identity(&self) -> &RuntimeSearchTelemetryIdentity {
        &self.identity
    }

    fn record_phase(
        &self,
        owner: RuntimeSearchTelemetryPhaseOwner,
        phase: &str,
        elapsed_micros: u64,
        budget_micros: u64,
    ) -> Result<RuntimePerformanceObservation, RuntimeSearchTelemetryError> {
        if runtime_search_telemetry_phase_owner(phase) != Some(owner) {
            return Err(RuntimeSearchTelemetryError::new(
                "runtime-search-telemetry-phase-owner-mismatch",
            ));
        }
        let mut state = self.state.lock().map_err(|_| {
            RuntimeSearchTelemetryError::new("runtime-search-telemetry-trace-poisoned")
        })?;
        if state.next_phase >= RUNTIME_SEARCH_TELEMETRY_PHASES.len()
            || RUNTIME_SEARCH_TELEMETRY_PHASES[state.next_phase] != phase
        {
            return Err(RuntimeSearchTelemetryError::new(
                "runtime-search-telemetry-phase-order",
            ));
        }
        let observation = RuntimePerformanceObservation::for_search_phase(
            &self.identity,
            phase,
            elapsed_micros,
            budget_micros,
            if elapsed_micros <= budget_micros {
                "within-budget"
            } else {
                "budget-exceeded"
            },
        )?;
        state.observations.push(observation.clone());
        state.next_phase += 1;
        Ok(observation)
    }

    pub fn record_client_phase(
        &self,
        phase: &str,
        elapsed_micros: u64,
        budget_micros: u64,
    ) -> Result<RuntimePerformanceObservation, RuntimeSearchTelemetryError> {
        self.record_phase(
            RuntimeSearchTelemetryPhaseOwner::Client,
            phase,
            elapsed_micros,
            budget_micros,
        )
    }

    pub fn record_server_admission_queue(
        &self,
        elapsed_micros: u64,
        budget_micros: u64,
    ) -> Result<RuntimePerformanceObservation, RuntimeSearchTelemetryError> {
        self.record_phase(
            RuntimeSearchTelemetryPhaseOwner::Admission,
            "server-admission-queue",
            elapsed_micros,
            budget_micros,
        )
    }

    pub fn record_snapshot_resolve(
        &self,
        elapsed_micros: u64,
        budget_micros: u64,
    ) -> Result<RuntimePerformanceObservation, RuntimeSearchTelemetryError> {
        self.record_phase(
            RuntimeSearchTelemetryPhaseOwner::RuntimeRoute,
            "snapshot-resolve",
            elapsed_micros,
            budget_micros,
        )
    }

    pub fn record_provider_dispatch(
        &self,
        elapsed_micros: u64,
        budget_micros: u64,
    ) -> Result<RuntimePerformanceObservation, RuntimeSearchTelemetryError> {
        self.record_phase(
            RuntimeSearchTelemetryPhaseOwner::RuntimeRoute,
            "provider-dispatch",
            elapsed_micros,
            budget_micros,
        )
    }

    pub fn record_search_execution(
        &self,
        elapsed_micros: u64,
        budget_micros: u64,
    ) -> Result<RuntimePerformanceObservation, RuntimeSearchTelemetryError> {
        self.record_phase(
            RuntimeSearchTelemetryPhaseOwner::SearchExecution,
            "parse-index-query",
            elapsed_micros,
            budget_micros,
        )
    }

    pub fn record_search_projection(
        &self,
        elapsed_micros: u64,
        budget_micros: u64,
    ) -> Result<RuntimePerformanceObservation, RuntimeSearchTelemetryError> {
        self.record_phase(
            RuntimeSearchTelemetryPhaseOwner::SearchProjection,
            "projection-rank",
            elapsed_micros,
            budget_micros,
        )
    }

    pub fn record_response_serialized(
        &self,
        elapsed_micros: u64,
        budget_micros: u64,
    ) -> Result<RuntimePerformanceObservation, RuntimeSearchTelemetryError> {
        self.record_phase(
            RuntimeSearchTelemetryPhaseOwner::ResponseService,
            "schema-validate-serialize",
            elapsed_micros,
            budget_micros,
        )
    }

    pub fn record_transport_terminal_egress(
        &self,
        terminal_state: &str,
        elapsed_micros: u64,
        budget_micros: u64,
    ) -> Result<RuntimePerformanceObservation, RuntimeSearchTelemetryError> {
        let mut state = self.state.lock().map_err(|_| {
            RuntimeSearchTelemetryError::new("runtime-search-telemetry-trace-poisoned")
        })?;
        if state.terminal_count != 0 {
            return Err(RuntimeSearchTelemetryError::new(
                "runtime-search-terminal-duplicate",
            ));
        }
        if state.next_phase >= RUNTIME_SEARCH_TELEMETRY_PHASES.len() {
            return Err(RuntimeSearchTelemetryError::new(
                "runtime-search-telemetry-phase-order",
            ));
        }
        let mut observation = RuntimePerformanceObservation::for_search_phase(
            &self.identity,
            "terminal-egress",
            elapsed_micros,
            budget_micros,
            if elapsed_micros <= budget_micros {
                "within-budget"
            } else {
                "budget-exceeded"
            },
        )?;
        state.next_phase = RUNTIME_SEARCH_TELEMETRY_PHASES.len();
        state.terminal_count = 1;
        if terminal_state != "ready" && terminal_state != "accepted" {
            observation.failure_reason = Some(terminal_state.to_owned());
        }
        state.observations.push(observation.clone());
        Ok(observation)
    }

    pub fn observations(&self) -> Vec<RuntimePerformanceObservation> {
        self.state
            .lock()
            .map(|state| state.observations.clone())
            .unwrap_or_default()
    }

    pub fn terminal_count(&self) -> usize {
        self.state
            .lock()
            .map(|state| state.terminal_count)
            .unwrap_or(0)
    }
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimePerformanceObservation {
    schema_id: String,
    schema_version: String,
    pub surface: String,
    pub stage: String,
    pub workspace_identity: Option<String>,
    pub session_id: Option<String>,
    pub request_id: Option<String>,
    pub language_id: Option<String>,
    pub provider_id: Option<String>,
    pub language_ids: Option<Vec<String>>,
    pub provider_ids: Option<Vec<String>>,
    pub workspace_snapshot_digest: Option<String>,
    pub source_snapshot_digest: Option<String>,
    pub source_index_digest: Option<String>,
    pub generation_digest: Option<String>,
    pub runtime_artifact_digest: Option<String>,
    pub runtime_bundle_digest: Option<String>,
    pub execution_publication_digest: Option<String>,
    pub provider_catalog_digest: Option<String>,
    pub transport_contract_digest: Option<String>,
    pub operation_id: Option<String>,
    pub event_identity: Option<String>,
    pub incident_id: Option<String>,
    pub incident_state: Option<String>,
    pub incident_transition: Option<String>,
    pub incident_transition_sequence: Option<u64>,
    pub requested_projection: Option<String>,
    pub observed_at_unix_micros: Option<u64>,
    pub process_resident_bytes: Option<u64>,
    pub process_peak_resident_bytes: Option<u64>,
    pub process_memory_budget_bytes: Option<u64>,
    pub process_memory_budget_status: Option<String>,
    pub process_disk_read_bytes: Option<u64>,
    pub process_disk_write_bytes: Option<u64>,
    pub process_page_ins: Option<u64>,
    pub process_open_descriptors: Option<u64>,
    pub process_open_descriptor_budget: Option<u64>,
    pub process_open_descriptor_budget_status: Option<String>,
    pub runtime_event_loop_lag_micros: Option<u64>,
    pub runtime_event_loop_lag_budget_micros: Option<u64>,
    pub runtime_event_loop_lag_budget_status: Option<String>,
    pub runtime_worker_threads: Option<u64>,
    pub runtime_alive_tasks: Option<u64>,
    pub runtime_global_queue_depth: Option<u64>,
    pub runtime_active_connections: Option<u64>,
    pub runtime_connection_limit: Option<u64>,
    pub runtime_connection_high_watermark: Option<u64>,
    pub runtime_rejected_connections: Option<u64>,
    pub runtime_diagnostic_queue_depth: Option<u64>,
    pub runtime_diagnostic_queue_capacity: Option<u64>,
    pub runtime_dropped_diagnostics: Option<u64>,
    pub owner_count: Option<u64>,
    pub selector_count: Option<u64>,
    pub relation_count: Option<u64>,
    pub source_bytes: Option<u64>,
    pub projection_bytes: Option<u64>,
    pub canonical_encoded_bytes: Option<u64>,
    pub memory_search_mapped_bytes: Option<u64>,
    pub memory_search_directory_bytes_validated: Option<u64>,
    pub memory_search_key_bytes_touched: Option<u64>,
    pub memory_search_value_bytes_touched: Option<u64>,
    pub memory_search_source_bytes_read: Option<u64>,
    pub memory_search_turso_opens: Option<u64>,
    pub memory_search_socket_connects: Option<u64>,
    pub memory_search_provider_spawns: Option<u64>,
    pub elapsed_micros: u64,
    pub budget_micros: u64,
    pub budget_status: String,
    pub failure_reason: Option<String>,
    pub retry_after_ms: Option<u64>,
}

/// Typed lifecycle transition projected into the resident performance writer.
#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeLifecycleEvent {
    pub owner_epoch: u64,
    pub workspace_identity: Option<String>,
    pub generation_digest: Option<String>,
    pub candidate_digest: Option<String>,
    pub transition: String,
    pub state: String,
    pub elapsed_micros: u64,
    pub read_bytes: u64,
    pub retained_bytes: u64,
    pub active_task_count: u64,
    pub active_child_count: u64,
}

impl RuntimeLifecycleEvent {
    pub fn into_observation(self) -> RuntimePerformanceObservation {
        let mut observation = RuntimePerformanceObservation::new(
            "runtime-server",
            "lifecycle",
            self.elapsed_micros,
            250_000,
            self.state.clone(),
        );
        observation.workspace_identity = self.workspace_identity;
        observation.generation_digest = self.generation_digest;
        observation.event_identity = Some(match self.candidate_digest {
            Some(candidate_digest) => format!(
                "owner-{}:{}:candidate={candidate_digest}",
                self.owner_epoch, self.transition
            ),
            None => format!("owner-{}:{}", self.owner_epoch, self.transition),
        });
        observation.source_bytes = Some(self.read_bytes);
        observation.projection_bytes = Some(self.retained_bytes);
        observation.runtime_alive_tasks = Some(self.active_task_count);
        observation.runtime_active_connections = Some(self.active_child_count);
        observation
    }
}

impl RuntimePerformanceObservation {
    pub fn new(
        surface: impl Into<String>,
        stage: impl Into<String>,
        elapsed_micros: u64,
        budget_micros: u64,
        budget_status: impl Into<String>,
    ) -> Self {
        Self {
            schema_id: "agent.semantic-protocols.runtime-server-performance-observation".to_owned(),
            schema_version: "1".to_owned(),
            surface: surface.into(),
            stage: stage.into(),
            workspace_identity: None,
            session_id: None,
            request_id: None,
            language_id: None,
            provider_id: None,
            language_ids: None,
            provider_ids: None,
            workspace_snapshot_digest: None,
            source_snapshot_digest: None,
            source_index_digest: None,
            generation_digest: None,
            runtime_artifact_digest: None,
            runtime_bundle_digest: None,
            execution_publication_digest: None,
            provider_catalog_digest: None,
            transport_contract_digest: None,
            operation_id: None,
            event_identity: None,
            incident_id: None,
            incident_state: None,
            incident_transition: None,
            incident_transition_sequence: None,
            requested_projection: None,
            observed_at_unix_micros: None,
            process_resident_bytes: None,
            process_peak_resident_bytes: None,
            process_memory_budget_bytes: None,
            process_memory_budget_status: None,
            process_disk_read_bytes: None,
            process_disk_write_bytes: None,
            process_page_ins: None,
            process_open_descriptors: None,
            process_open_descriptor_budget: None,
            process_open_descriptor_budget_status: None,
            runtime_event_loop_lag_micros: None,
            runtime_event_loop_lag_budget_micros: None,
            runtime_event_loop_lag_budget_status: None,
            runtime_worker_threads: None,
            runtime_alive_tasks: None,
            runtime_global_queue_depth: None,
            runtime_active_connections: None,
            runtime_connection_limit: None,
            runtime_connection_high_watermark: None,
            runtime_rejected_connections: None,
            runtime_diagnostic_queue_depth: None,
            runtime_diagnostic_queue_capacity: None,
            runtime_dropped_diagnostics: None,
            owner_count: None,
            selector_count: None,
            relation_count: None,
            source_bytes: None,
            projection_bytes: None,
            canonical_encoded_bytes: None,
            memory_search_mapped_bytes: None,
            memory_search_directory_bytes_validated: None,
            memory_search_key_bytes_touched: None,
            memory_search_value_bytes_touched: None,
            memory_search_source_bytes_read: None,
            memory_search_turso_opens: None,
            memory_search_socket_connects: None,
            memory_search_provider_spawns: None,
            elapsed_micros,
            budget_micros,
            budget_status: budget_status.into(),
            failure_reason: None,
            retry_after_ms: None,
        }
    }

    pub fn for_search_phase(
        identity: &RuntimeSearchTelemetryIdentity,
        phase: &str,
        elapsed_micros: u64,
        budget_micros: u64,
        budget_status: impl Into<String>,
    ) -> Result<Self, RuntimeSearchTelemetryError> {
        if !RUNTIME_SEARCH_TELEMETRY_PHASES.contains(&phase) {
            return Err(RuntimeSearchTelemetryError::new(
                "runtime-search-telemetry-phase-order",
            ));
        }
        let mut observation = Self::new(
            "search-query-playbook",
            phase,
            elapsed_micros,
            budget_micros,
            budget_status,
        );
        observation.workspace_identity = Some(identity.workspace_identity.clone());
        observation.session_id = Some(identity.session_id.clone());
        observation.request_id = Some(identity.request_id.clone());
        observation.language_ids = Some(identity.language_ids.clone());
        observation.provider_ids = Some(identity.provider_ids.clone());
        if let [language_id] = identity.language_ids.as_slice() {
            observation.language_id = Some(language_id.clone());
        }
        if let [provider_id] = identity.provider_ids.as_slice() {
            observation.provider_id = Some(provider_id.clone());
        }
        observation.workspace_snapshot_digest = Some(identity.workspace_snapshot_digest.clone());
        observation.source_snapshot_digest = Some(identity.source_snapshot_digest.clone());
        observation.source_index_digest = Some(identity.source_index_digest.clone());
        observation.generation_digest = Some(identity.source_generation_digest.clone());
        observation.runtime_artifact_digest = Some(identity.runtime_artifact_digest.clone());
        observation.runtime_bundle_digest = Some(identity.runtime_bundle_digest.clone());
        observation.execution_publication_digest =
            Some(identity.execution_publication_digest.clone());
        observation.provider_catalog_digest = Some(identity.provider_catalog_digest.clone());
        observation.operation_id = Some(identity.request_id.clone());
        Ok(observation)
    }

    /// Seals one canonical budget failure for resident de-duplication and
    /// durable Turso audit. Observation time is deliberately excluded from the
    /// identity so duplicate render/transport paths converge on one event.
    pub fn seal_budget_failure_identity(&mut self) {
        self.observed_at_unix_micros.get_or_insert_with(|| {
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|duration| u64::try_from(duration.as_micros()).unwrap_or(u64::MAX))
                .unwrap_or(0)
        });
        if self.event_identity.is_none() {
            self.event_identity = Some(
                serde_json::json!([
                    self.workspace_identity,
                    self.surface,
                    self.stage,
                    self.language_id,
                    self.generation_digest,
                    self.operation_id,
                    self.elapsed_micros,
                    self.budget_micros,
                    self.budget_status,
                    self.failure_reason,
                ])
                .to_string(),
            );
        }
    }

    pub(super) fn record_runtime_process_memory(
        &mut self,
        memory: super::process_memory::ProcessMemoryObservation,
    ) {
        self.process_resident_bytes = memory.resident_bytes;
        self.process_peak_resident_bytes = memory.peak_resident_bytes;
        self.process_memory_budget_bytes = Some(memory.budget_bytes);
        self.process_memory_budget_status = Some(memory.budget_status().to_owned());
        self.process_disk_read_bytes = memory.disk_read_bytes;
        self.process_disk_write_bytes = memory.disk_write_bytes;
        self.process_page_ins = memory.page_ins;
        self.process_open_descriptors = memory.open_descriptors;
        self.process_open_descriptor_budget = Some(memory.open_descriptor_budget);
        self.process_open_descriptor_budget_status =
            Some(memory.open_descriptor_budget_status().to_owned());
        self.runtime_event_loop_lag_micros = Some(memory.event_loop_lag_micros);
        self.runtime_event_loop_lag_budget_micros = Some(memory.event_loop_lag_budget_micros);
        self.runtime_event_loop_lag_budget_status =
            Some(memory.event_loop_lag_budget_status().to_owned());
        self.runtime_worker_threads = Some(memory.runtime_worker_threads);
        self.runtime_alive_tasks = Some(memory.runtime_alive_tasks);
        self.runtime_global_queue_depth = Some(memory.runtime_global_queue_depth);
        if memory.budget_exceeded() && self.failure_reason.is_none() {
            self.failure_reason = Some("runtime-server-memory-budget-exceeded".to_owned());
        }
        if memory.event_loop_lag_budget_exceeded() && self.failure_reason.is_none() {
            self.failure_reason = Some("runtime-server-event-loop-lag-budget-exceeded".to_owned());
        }
        if memory.open_descriptor_budget_exceeded() && self.failure_reason.is_none() {
            self.failure_reason = Some("runtime-server-open-descriptor-budget-exceeded".to_owned());
        }
    }

    pub fn with_materialization_metrics(
        mut self,
        owner_count: u64,
        selector_count: u64,
        relation_count: u64,
        source_bytes: u64,
        projection_bytes: u64,
    ) -> Self {
        self.owner_count = Some(owner_count);
        self.selector_count = Some(selector_count);
        self.relation_count = Some(relation_count);
        self.source_bytes = Some(source_bytes);
        self.projection_bytes = Some(projection_bytes);
        self
    }

    pub fn with_memory_search_metrics(
        mut self,
        mapped_bytes: u64,
        directory_bytes_validated: u64,
        key_bytes_touched: u64,
        value_bytes_touched: u64,
        source_bytes_read: u64,
    ) -> Self {
        self.memory_search_mapped_bytes = Some(mapped_bytes);
        self.memory_search_directory_bytes_validated = Some(directory_bytes_validated);
        self.memory_search_key_bytes_touched = Some(key_bytes_touched);
        self.memory_search_value_bytes_touched = Some(value_bytes_touched);
        self.memory_search_source_bytes_read = Some(source_bytes_read);
        self.memory_search_turso_opens = Some(0);
        self.memory_search_socket_connects = Some(0);
        self.memory_search_provider_spawns = Some(0);
        self
    }

    pub fn with_operation_id(mut self, operation_id: impl Into<String>) -> Self {
        self.operation_id = Some(operation_id.into());
        self
    }
}
pub(super) fn push_optional(
    attributes: &mut Vec<opentelemetry::KeyValue>,
    key: &'static str,
    value: Option<String>,
) {
    if let Some(value) = value.filter(|value| !value.is_empty()) {
        attributes.push(opentelemetry::KeyValue::new(key, value));
    }
}

pub(super) fn push_optional_u64(
    attributes: &mut Vec<opentelemetry::KeyValue>,
    key: &'static str,
    value: Option<u64>,
) {
    if let Some(value) = value {
        attributes.push(opentelemetry::KeyValue::new(
            key,
            i64::try_from(value).unwrap_or(i64::MAX),
        ));
    }
}
