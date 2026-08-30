use std::path::PathBuf;

use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct QualificationPlan {
    pub(super) schema_id: String,
    pub(super) schema_version: String,
    pub(super) resident_sample_count: usize,
    pub(super) lock_path: PathBuf,
    pub(super) required_languages: Vec<String>,
    pub(super) client_protocol: ClientProtocolContract,
    pub(super) cases: Vec<QualificationCase>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct ClientProtocolContract {
    pub(super) protocol_id: String,
    pub(super) protocol_version: String,
    pub(super) transport: String,
    pub(super) phases: Vec<String>,
    pub(super) required_telemetry_events: Vec<String>,
    pub(super) applies_to_case_count: usize,
    pub(super) maximum_resident_micros: u64,
    pub(super) session_policy: String,
    pub(super) ready_effects: Vec<String>,
    pub(super) forbidden_ready_effects: Vec<String>,
    pub(super) non_ready_dispatch_count: usize,
    pub(super) residual_task_count: usize,
    pub(super) p50_maximum_micros: u64,
    pub(super) p99_maximum_micros: u64,
    pub(super) max_maximum_micros: u64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct QualificationCase {
    pub(super) case_id: String,
    pub(super) resource_id: String,
    pub(super) scenario_id: String,
    pub(super) language_id: String,
    pub(super) provider_id: String,
    pub(super) search: QualificationSearch,
    pub(super) query: QualificationQuery,
    pub(super) zero_match_terms: Vec<String>,
    pub(super) required_telemetry_events: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct QualificationSearch {
    pub(super) method: String,
    pub(super) terms: Vec<String>,
    pub(super) view: String,
    pub(super) minimum_candidates: usize,
    pub(super) maximum_resident_micros: u64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct QualificationQuery {
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
    pub(super) client_protocol: ClientProtocolReceipt,
    pub(super) qualified_case_count: usize,
    pub(super) cases: Vec<QualificationCaseReceipt>,
    pub(super) status: &'static str,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub(super) struct ClientProtocolReceipt {
    pub(super) protocol_id: &'static str,
    pub(super) protocol_version: &'static str,
    pub(super) transport: &'static str,
    pub(super) application_api: &'static str,
    pub(super) routes: [&'static str; 2],
    pub(super) terminal_outcomes: [&'static str; 5],
    pub(super) qualified_case_count: usize,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct QualificationCaseReceipt {
    pub(super) case_id: String,
    pub(super) resource_id: String,
    pub(super) scenario_id: String,
    pub(super) language_id: String,
    pub(super) provider_id: String,
    pub(super) revision: String,
    pub(super) git_tree: String,
    pub(super) generation_digest: String,
    pub(super) root_digest: String,
    pub(super) search_operation_id: String,
    pub(super) search_elapsed_micros: u64,
    pub(super) candidate_count: usize,
    pub(super) selector: String,
    pub(super) query_operation_id: String,
    pub(super) query_elapsed_micros: u64,
    pub(super) callable_skeleton_operation_id: String,
    pub(super) callable_skeleton_elapsed_micros: u64,
    pub(super) zero_match_operation_id: String,
    pub(super) route: &'static str,
    pub(super) search_terminal: &'static str,
    pub(super) query_terminal: &'static str,
    pub(super) callable_skeleton_terminal: &'static str,
    pub(super) zero_match_terminal: &'static str,
    pub(super) status: &'static str,
}
