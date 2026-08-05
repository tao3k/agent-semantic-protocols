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
    pub process_resident_bytes: Option<u64>,
    pub process_peak_resident_bytes: Option<u64>,
    pub process_memory_budget_bytes: Option<u64>,
    pub process_memory_budget_status: Option<String>,
    pub process_disk_read_bytes: Option<u64>,
    pub process_disk_write_bytes: Option<u64>,
    pub process_page_ins: Option<u64>,
    pub runtime_event_loop_lag_micros: Option<u64>,
    pub runtime_event_loop_lag_budget_micros: Option<u64>,
    pub runtime_event_loop_lag_budget_status: Option<String>,
    pub owner_count: Option<u64>,
    pub selector_count: Option<u64>,
    pub relation_count: Option<u64>,
    pub source_bytes: Option<u64>,
    pub projection_bytes: Option<u64>,
    pub canonical_encoded_bytes: Option<u64>,
    pub elapsed_micros: u64,
    pub budget_micros: u64,
    pub budget_status: String,
    pub failure_reason: Option<String>,
    pub retry_after_ms: Option<u64>,
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
            process_resident_bytes: None,
            process_peak_resident_bytes: None,
            process_memory_budget_bytes: None,
            process_memory_budget_status: None,
            process_disk_read_bytes: None,
            process_disk_write_bytes: None,
            process_page_ins: None,
            runtime_event_loop_lag_micros: None,
            runtime_event_loop_lag_budget_micros: None,
            runtime_event_loop_lag_budget_status: None,
            owner_count: None,
            selector_count: None,
            relation_count: None,
            source_bytes: None,
            projection_bytes: None,
            canonical_encoded_bytes: None,
            elapsed_micros,
            budget_micros,
            budget_status: budget_status.into(),
            failure_reason: None,
            retry_after_ms: None,
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
        self.runtime_event_loop_lag_micros = Some(memory.event_loop_lag_micros);
        self.runtime_event_loop_lag_budget_micros = Some(memory.event_loop_lag_budget_micros);
        self.runtime_event_loop_lag_budget_status =
            Some(memory.event_loop_lag_budget_status().to_owned());
        if memory.budget_exceeded() && self.failure_reason.is_none() {
            self.failure_reason = Some("runtime-server-memory-budget-exceeded".to_owned());
        }
        if memory.event_loop_lag_budget_exceeded() && self.failure_reason.is_none() {
            self.failure_reason = Some("runtime-server-event-loop-lag-budget-exceeded".to_owned());
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

    pub fn with_operation_id(mut self, operation_id: impl Into<String>) -> Self {
        self.operation_id = Some(operation_id.into());
        self
    }
}
