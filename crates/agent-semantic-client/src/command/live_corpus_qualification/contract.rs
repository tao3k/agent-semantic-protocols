// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use std::path::PathBuf;

use serde::Deserialize;
use serde::Serialize;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct QualificationPlan {
    pub(super) schema_id: String,
    pub(super) schema_version: String,
    pub(super) resident_sample_count: usize,
    pub(super) sequential_sample_count: usize,
    pub(super) concurrent_sample_count: usize,
    pub(super) failure_injections: Vec<String>,
    pub(super) cache_states: Vec<QualificationCacheState>,
    pub(super) lock_path: PathBuf,
    pub(super) required_languages: Vec<String>,
    pub(super) client_protocol: ClientProtocolContract,
    pub(super) cases: Vec<QualificationCase>,
}

#[derive(Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct QualificationCacheState {
    pub(super) state: String,
    pub(super) prepare_action: String,
    pub(super) mutation_scope: String,
    pub(super) sample_count: usize,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct ClientProtocolContract {
    pub(super) protocol_id: String,
    pub(super) protocol_version: String,
    pub(super) transport: String,
    pub(super) workspace_scheduling: String,
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

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct QualificationCase {
    pub(super) case_id: String,
    pub(super) resource_id: String,
    pub(super) scenario_id: String,
    pub(super) language_id: String,
    pub(super) provider_id: String,
    pub(super) search: QualificationSearch,
    pub(super) query: QualificationQuery,
    pub(super) zero_match_search: QualificationSearch,
    pub(super) required_telemetry_events: Vec<String>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct QualificationSearch {
    pub(super) rg: Vec<String>,
    pub(super) tantivy: Vec<String>,
    pub(super) minimum_candidates: usize,
    pub(super) maximum_search_micros: u64,
}

#[derive(Clone, Debug, Deserialize)]
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
    pub(super) workspace_scheduling: &'static str,
    pub(super) concurrent_workspace_count: usize,
    pub(super) phases: [&'static str; 6],
    pub(super) session_policy: &'static str,
    pub(super) ready_effects: [&'static str; 4],
    pub(super) forbidden_ready_effects: [&'static str; 6],
    pub(super) non_ready_dispatch_count: usize,
    pub(super) residual_task_count: usize,
    pub(super) cancel_outcome: &'static str,
    pub(super) request_outcome: &'static str,
    pub(super) required_telemetry_events: [&'static str; 6],
    pub(super) maximum_resident_micros: u64,
    pub(super) p50_maximum_micros: u64,
    pub(super) p99_maximum_micros: u64,
    pub(super) max_maximum_micros: u64,
    pub(super) qualified_case_count: usize,
}

#[derive(Debug, Serialize, Clone, Eq, PartialEq)]
#[serde(rename_all = "camelCase")]
pub(super) struct LatencyDistribution {
    pub(super) sample_count: usize,
    pub(super) min_micros: u64,
    pub(super) p50_micros: u64,
    pub(super) p95_micros: u64,
    pub(super) p99_micros: u64,
    pub(super) max_micros: u64,
}

impl LatencyDistribution {
    pub(super) fn from_samples(mut samples: Vec<u64>) -> Result<Self, String> {
        if samples.is_empty() {
            return Err("Live Corpus latency distribution requires samples".to_owned());
        }
        samples.sort_unstable();
        let percentile = |numerator: usize| {
            let rank = samples.len().saturating_mul(numerator).saturating_add(99) / 100;
            samples[rank.saturating_sub(1).min(samples.len() - 1)]
        };
        Ok(Self {
            sample_count: samples.len(),
            min_micros: samples[0],
            p50_micros: percentile(50),
            p95_micros: percentile(95),
            p99_micros: percentile(99),
            max_micros: samples[samples.len() - 1],
        })
    }
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
    pub(super) source_artifact_digest: String,
    pub(super) benchmark_workspace_identity: String,
    pub(super) benchmark_workspace_materialization: String,
    pub(super) benchmark_workspace_materialization_elapsed_micros: u64,
    pub(super) benchmark_workspace_retained: bool,
    pub(super) benchmark_workspace_cleanup_elapsed_micros: u64,
    pub(super) release_prepare_elapsed_micros: u64,
    pub(super) cancellation_probe_elapsed_micros: u64,
    pub(super) backpressure_capacity: usize,
    pub(super) backpressure_held_call_count: usize,
    pub(super) backpressure_rejected_call_count: usize,
    pub(super) backpressure_probe_elapsed_micros: u64,
    pub(super) stale_content_binding_rejected: bool,
    pub(super) stale_content_binding_probe_elapsed_micros: u64,
    pub(super) generation_digest: String,
    pub(super) root_digest: String,
    pub(super) search_operation_id: String,
    pub(super) search_elapsed_micros: u64,
    pub(super) resident_sample_count: usize,
    pub(super) search_total_latency_micros: LatencyDistribution,
    pub(super) candidate_count: usize,
    pub(super) selector: String,
    pub(super) query_operation_id: String,
    pub(super) query_elapsed_micros: u64,
    pub(super) exact_source_latency_micros: LatencyDistribution,
    pub(super) callable_skeleton_operation_id: String,
    pub(super) callable_skeleton_elapsed_micros: u64,
    pub(super) callable_skeleton_latency_micros: LatencyDistribution,
    pub(super) cold_build_sample_count: usize,
    pub(super) cold_build_search_query_latency_micros: LatencyDistribution,
    pub(super) cold_load_sample_count: usize,
    pub(super) cold_load_prepare_latency_micros: LatencyDistribution,
    pub(super) cold_load_search_query_latency_micros: LatencyDistribution,
    pub(super) warm_read_prepare_elapsed_micros: u64,
    pub(super) sequential_sample_count: usize,
    pub(super) sequential_search_query_latency_micros: LatencyDistribution,
    pub(super) concurrent_sample_count: usize,
    pub(super) concurrent_search_query_latency_micros: LatencyDistribution,
    pub(super) merkle_owner_path: String,
    pub(super) merkle_source_blob_digest: String,
    pub(super) merkle_owner_subtree_digest: String,
    pub(super) merkle_proof_digest: String,
    pub(super) merkle_proof_step_count: usize,
    pub(super) zero_match_operation_id: String,
    pub(super) runtime_ecosystem: &'static str,
    pub(super) search_execution_mode: &'static str,
    pub(super) search_binding: serde_json::Value,
    pub(super) exact_read_mode: &'static str,
    pub(super) exact_read_work_counters: serde_json::Value,
    pub(super) route: &'static str,
    pub(super) search_terminal: &'static str,
    pub(super) query_terminal: &'static str,
    pub(super) callable_skeleton_terminal: &'static str,
    pub(super) zero_match_terminal: &'static str,
    pub(super) semantic_projection_schema_id: String,
    pub(super) payload_schema_id: String,
    pub(super) payload_digest: String,
    pub(super) status: &'static str,
}
