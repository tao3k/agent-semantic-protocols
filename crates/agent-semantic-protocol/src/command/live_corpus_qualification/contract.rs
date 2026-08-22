use std::path::PathBuf;

use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct QualificationPlanV1 {
    pub(super) schema_id: String,
    pub(super) schema_version: String,
    pub(super) lock_path: PathBuf,
    pub(super) required_languages: Vec<String>,
    pub(super) resident_sample_count: usize,
    pub(super) cases: Vec<QualificationCaseV1>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct QualificationCaseV1 {
    pub(super) case_id: String,
    pub(super) resource_id: String,
    pub(super) language_id: String,
    pub(super) provider_id: String,
    pub(super) search: QualificationSearchV1,
    pub(super) query: QualificationQueryV1,
    pub(super) zero_match_terms: Vec<String>,
    pub(super) required_telemetry_events: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct QualificationSearchV1 {
    pub(super) method: String,
    pub(super) terms: Vec<String>,
    pub(super) view: String,
    pub(super) minimum_candidates: usize,
    pub(super) maximum_resident_micros: u64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct QualificationQueryV1 {
    pub(super) selector_strategy: String,
    pub(super) owner_view: String,
    pub(super) projection_scope: String,
    pub(super) maximum_resident_micros: u64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct QualificationReceipt {
    pub(super) schema_id: &'static str,
    pub(super) schema_version: &'static str,
    pub(super) plan_digest: String,
    pub(super) lock_digest: String,
    pub(super) qualified_case_count: usize,
    pub(super) cases: Vec<QualificationCaseReceipt>,
    pub(super) status: &'static str,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct QualificationCaseReceipt {
    pub(super) case_id: String,
    pub(super) resource_id: String,
    pub(super) language_id: String,
    pub(super) provider_id: String,
    pub(super) revision: String,
    pub(super) git_tree: String,
    pub(super) generation_digest: String,
    pub(super) root_digest: String,
    pub(super) search_operation_id: String,
    pub(super) search_elapsed_micros: u64,
    pub(super) resident_sample_count: usize,
    pub(super) search_resident_read_latency_micros:
        super::resident_metrics::ResidentLatencyDistributionV1,
    pub(super) search_service_latency_micros:
        super::resident_metrics::ResidentLatencyDistributionV1,
    pub(super) search_total_latency_micros: super::resident_metrics::ResidentLatencyDistributionV1,
    pub(super) candidate_count: usize,
    pub(super) selector: String,
    pub(super) query_operation_id: String,
    pub(super) query_elapsed_micros: u64,
    pub(super) exact_source_latency_micros: super::resident_metrics::ResidentLatencyDistributionV1,
    pub(super) callable_skeleton_latency_micros:
        super::resident_metrics::ResidentLatencyDistributionV1,
    pub(super) merkle_owner_path: String,
    pub(super) merkle_source_blob_digest: String,
    pub(super) merkle_owner_subtree_digest: String,
    pub(super) merkle_proof_digest: String,
    pub(super) merkle_proof_step_count: usize,
    pub(super) zero_match_operation_id: String,
    pub(super) source_exact_telemetry_digest: String,
    pub(super) source_index_telemetry_digest: String,
    pub(super) callable_skeleton_telemetry_digest: String,
    pub(super) merkle_telemetry_digest: String,
    pub(super) runtime_ecosystem: &'static str,
    pub(super) search_read_mode: &'static str,
    pub(super) search_read_work_counters:
        agent_semantic_client_db::runtime_resident_read::RuntimeResidentReadWorkCounters,
    pub(super) exact_read_mode: &'static str,
    pub(super) exact_read_work_counters:
        agent_semantic_client_db::runtime_resident_read::RuntimeResidentReadWorkCounters,
    pub(super) status: &'static str,
    pub(super) semantic_projection_schema_id: String,
    pub(super) payload_schema_id: String,
    pub(super) payload_digest: String,
}
