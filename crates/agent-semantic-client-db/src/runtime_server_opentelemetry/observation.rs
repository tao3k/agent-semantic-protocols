// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-only

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

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RuntimeSearchTelemetryIdentity {
    pub workspace_identity: String,
    pub source_generation_digest: String,
    pub runtime_artifact_digest: String,
    pub provider_id: String,
    pub request_id: String,
}

impl RuntimeSearchTelemetryIdentity {
    pub fn new(
        workspace_identity: impl Into<String>,
        source_generation_digest: impl Into<String>,
        runtime_artifact_digest: impl Into<String>,
        provider_id: impl Into<String>,
        request_id: impl Into<String>,
    ) -> Result<Self, RuntimeSearchTelemetryError> {
        let identity = Self {
            workspace_identity: workspace_identity.into(),
            source_generation_digest: source_generation_digest.into(),
            runtime_artifact_digest: runtime_artifact_digest.into(),
            provider_id: provider_id.into(),
            request_id: request_id.into(),
        };
        if identity.workspace_identity.is_empty()
            || identity.source_generation_digest.is_empty()
            || identity.runtime_artifact_digest.is_empty()
            || identity.provider_id.is_empty()
            || identity.request_id.is_empty()
        {
            return Err(RuntimeSearchTelemetryError::new(
                "runtime-search-telemetry-identity-mismatch",
            ));
        }
        Ok(identity)
    }
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

#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimePerformanceObservation {
    schema_id: String,
    schema_version: String,
    pub surface: String,
    pub stage: String,
    pub workspace_identity: Option<String>,
    pub language_id: Option<String>,
    pub generation_digest: Option<String>,
    pub runtime_artifact_digest: Option<String>,
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
            language_id: None,
            generation_digest: None,
            runtime_artifact_digest: None,
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
