// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use std::path::PathBuf;

use base64::Engine;
use serde::Deserialize;
use serde::Serialize;

const AGENT_ORG_TOPOLOGY_PROMPT_V1: &str = "Analyze only the exact source and callable-skeleton evidence in this packet. Return one Agent Org Topology Contribution V1 with a concise summary, a valid Org document, and typed relationships touching the selected scope. Do not infer from filenames or mutate the base Search topology.";

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
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
#[serde(deny_unknown_fields)]
pub(super) struct QualificationCacheState {
    pub(super) state: String,
    pub(super) prepare_action: String,
    pub(super) mutation_scope: String,
    pub(super) sample_count: usize,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
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
#[serde(deny_unknown_fields)]
pub(super) struct QualificationCase {
    pub(super) case_id: String,
    pub(super) resource_id: String,
    pub(super) scenario_id: String,
    pub(super) language_id: String,
    pub(super) provider_id: String,
    pub(super) scenario_classes: Vec<String>,
    pub(super) search: String,
    pub(super) zero_match_search: String,
    pub(super) source_query: String,
    pub(super) callable_skeleton_query: String,
    pub(super) minimum_candidates: usize,
    pub(super) maximum_search_micros: u64,
    pub(super) maximum_resident_query_micros: u64,
    pub(super) required_telemetry_events: Vec<String>,
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
    pub(super) query_materialization_mode: &'static str,
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

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(in crate::command::live_corpus) struct AgentOrgTopologyEvidence {
    pub(in crate::command::live_corpus) schema_id: String,
    pub(in crate::command::live_corpus) schema_version: String,
    pub(in crate::command::live_corpus) case_id: String,
    pub(in crate::command::live_corpus) resource_id: String,
    pub(in crate::command::live_corpus) source_generation_digest: String,
    pub(in crate::command::live_corpus) base_topology_generation_digest: String,
    pub(in crate::command::live_corpus) selector: String,
    pub(in crate::command::live_corpus) source_query_operation_id: String,
    pub(in crate::command::live_corpus) source_bytes_base64: String,
    pub(in crate::command::live_corpus) callable_skeleton_operation_id: String,
    pub(in crate::command::live_corpus) callable_skeleton_bytes_base64: String,
    pub(in crate::command::live_corpus) evidence_digest: String,
    pub(in crate::command::live_corpus) prompt: String,
    pub(in crate::command::live_corpus) prompt_digest: String,
    pub(in crate::command::live_corpus) terminal: AgentOrgTopologyEvidenceTerminal,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(in crate::command::live_corpus) struct AgentOrgTopologyEvidenceTerminal {
    pub(in crate::command::live_corpus) state: String,
    pub(in crate::command::live_corpus) terminal_count: u64,
    pub(in crate::command::live_corpus) reason_kind: Option<String>,
}

impl AgentOrgTopologyEvidence {
    #[expect(
        clippy::too_many_arguments,
        reason = "the exact Query evidence identity must stay explicit at the Live Corpus boundary"
    )]
    pub(in crate::command::live_corpus) fn admit(
        case_id: String,
        resource_id: String,
        source_generation_digest: String,
        base_topology_generation_digest: String,
        selector: String,
        source_query_operation_id: String,
        source_bytes: &[u8],
        callable_skeleton_operation_id: String,
        callable_skeleton_bytes: &[u8],
    ) -> Result<Self, String> {
        for (field, value) in [
            ("sourceGenerationDigest", source_generation_digest.as_str()),
            (
                "baseTopologyGenerationDigest",
                base_topology_generation_digest.as_str(),
            ),
        ] {
            if value.len() != 75
                || !value.starts_with("blake3-256:")
                || !value[11..].bytes().all(|byte| byte.is_ascii_hexdigit())
            {
                return Err(format!(
                    "reasonKind=live-corpus-topology-evidence-binding-invalid field={field}"
                ));
            }
        }
        if case_id.is_empty()
            || resource_id.is_empty()
            || selector.is_empty()
            || source_query_operation_id.is_empty()
            || source_bytes.is_empty()
            || callable_skeleton_operation_id.is_empty()
            || callable_skeleton_bytes.is_empty()
        {
            return Err("reasonKind=live-corpus-topology-evidence-content-empty".to_owned());
        }
        let identity = serde_json::to_vec(&(
            &case_id,
            &resource_id,
            &source_generation_digest,
            &base_topology_generation_digest,
            &selector,
            &source_query_operation_id,
            source_bytes,
            &callable_skeleton_operation_id,
            callable_skeleton_bytes,
        ))
        .map_err(|error| format!("encode Live Corpus topology evidence identity: {error}"))?;
        Ok(Self {
            schema_id: "agent.semantic-protocols.agent-org-topology-evidence".to_owned(),
            schema_version: "1".to_owned(),
            case_id,
            resource_id,
            source_generation_digest,
            base_topology_generation_digest,
            selector,
            source_query_operation_id,
            source_bytes_base64: base64::engine::general_purpose::STANDARD.encode(source_bytes),
            callable_skeleton_operation_id,
            callable_skeleton_bytes_base64: base64::engine::general_purpose::STANDARD
                .encode(callable_skeleton_bytes),
            evidence_digest: format!("blake3-256:{}", blake3::hash(&identity).to_hex()),
            prompt: AGENT_ORG_TOPOLOGY_PROMPT_V1.to_owned(),
            prompt_digest: format!(
                "blake3-256:{}",
                blake3::hash(AGENT_ORG_TOPOLOGY_PROMPT_V1.as_bytes()).to_hex()
            ),
            terminal: AgentOrgTopologyEvidenceTerminal {
                state: "ready".to_owned(),
                terminal_count: 1,
                reason_kind: None,
            },
        })
    }

    pub(in crate::command::live_corpus) fn validate(&self) -> Result<(), String> {
        let source_bytes = base64::engine::general_purpose::STANDARD
            .decode(&self.source_bytes_base64)
            .map_err(|error| format!("decode Live Corpus source evidence: {error}"))?;
        let callable_skeleton_bytes = base64::engine::general_purpose::STANDARD
            .decode(&self.callable_skeleton_bytes_base64)
            .map_err(|error| format!("decode Live Corpus skeleton evidence: {error}"))?;
        let rebuilt = Self::admit(
            self.case_id.clone(),
            self.resource_id.clone(),
            self.source_generation_digest.clone(),
            self.base_topology_generation_digest.clone(),
            self.selector.clone(),
            self.source_query_operation_id.clone(),
            &source_bytes,
            self.callable_skeleton_operation_id.clone(),
            &callable_skeleton_bytes,
        )?;
        if rebuilt == *self {
            Ok(())
        } else {
            Err("reasonKind=live-corpus-topology-evidence-derived-identity-mismatch".to_owned())
        }
    }
}
