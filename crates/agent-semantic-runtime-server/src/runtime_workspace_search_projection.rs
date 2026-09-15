// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Graph fan-in and Project Topology settlement for progressive Search evidence.

use crate::RuntimeQueryGeneration;
use crate::runtime_asp_client::AspClientOperationError;
use crate::runtime_asp_client::workspace_search_playbook::{
    ProgressiveSearchEvidence, structural_candidate_owner_scope,
};

pub(super) struct ProgressiveSearchProjection {
    pub(super) result: serde_json::Value,
}

pub(super) async fn synthesize_progressive_search_projection(
    request_id: &str,
    language_id: &str,
    evidence: ProgressiveSearchEvidence,
    generation: &RuntimeQueryGeneration,
    project_root: &std::path::Path,
) -> Result<ProgressiveSearchProjection, AspClientOperationError> {
    let ProgressiveSearchEvidence {
        clause_receipts,
        syntax_candidates,
        graph_query_clauses,
        graph_relation_patterns,
        execution_budget,
        resident,
        topology_scope,
        retrieval_micros,
        owner_materialization_micros,
        structural_micros,
        retrieval_resource_receipt,
        structural_resource_receipt,
        resource_permits,
    } = evidence;
    execution_budget
        .validate_for_generation(generation.generation_digest())
        .map_err(AspClientOperationError::Message)?;
    let graph_candidate_owner_limit = execution_budget.graph_candidate_owner_limit();
    let evidence_item_limit = execution_budget.evidence_item_limit();
    let graph_started = std::time::Instant::now();
    let graph_fan_in = if graph_query_clauses.is_empty() {
        None
    } else {
        let (candidate_owners, candidate_frontier_truncated) =
            structural_candidate_owner_scope(&syntax_candidates, graph_candidate_owner_limit);
        let graph_generation = resident
            .build_graph_generation_for_owner_scope(&topology_scope)
            .map_err(AspClientOperationError::Message)?;
        let graph_evaluation_budget =
            execution_budget.graph_evaluation_budget_for(&graph_generation);
        let graph = crate::runtime_search_graph::evaluate_resident_workspace_playbook_graph(
            request_id,
            language_id,
            &graph_query_clauses,
            &graph_relation_patterns,
            &candidate_owners,
            graph_evaluation_budget,
            &resident,
            &graph_generation,
        )
        .map_err(|error| {
            AspClientOperationError::Terminal(
                agent_semantic_client_server::AspClientDispatchError {
                    reason_kind: error.reason_kind.to_owned(),
                    message: error.message,
                    details: error.details,
                },
            )
        })?;
        let truncated = candidate_frontier_truncated
            || graph_evaluation_budget.is_some_and(|budget| {
                candidate_owners.len() > budget.max_results
                    && graph.candidate_owner_ids.len() == budget.max_results
            });
        Some(agent_semantic_search::WorkspaceSearchGraphFanIn {
            ranked_candidate_owners: graph.candidate_owner_ids,
            applied_clause_count: graph_query_clauses.len(),
            complete: true,
            truncated,
        })
    };
    let graph_micros = graph_started.elapsed().as_micros();
    let result_started = std::time::Instant::now();
    let workspace_result = agent_semantic_search::synthesize_workspace_search_playbook_result(
        clause_receipts,
        syntax_candidates,
        graph_fan_in,
        evidence_item_limit,
    )?;
    let workspace_result = serde_json::to_value(workspace_result)
        .map_err(|error| AspClientOperationError::Message(error.to_string()))?;
    let result_micros = result_started.elapsed().as_micros();
    let topology_started = std::time::Instant::now();
    let attachment = generation
        .build_or_get_project_topology(project_root, &resident)
        .await
        .map_err(AspClientOperationError::Message)?;
    let topology_micros = topology_started.elapsed().as_micros();
    let settlement_started = std::time::Instant::now();
    let settlement =
        agent_semantic_search_projection::SearchTopologySettlement::from_workspace_result(
            request_id,
            &workspace_result,
            attachment.library(),
        )
        .map_err(|error| {
            AspClientOperationError::Terminal(
                agent_semantic_client_server::AspClientDispatchError {
                    reason_kind: error.reason_kind().to_owned(),
                    message: error.to_string(),
                    details: Some(serde_json::json!({
                        "failureStage": "search-topology-settlement",
                        "runtimeGenerationDigest": generation.generation_digest(),
                        "terminalCount": 1
                    })),
                },
            )
        })?;
    let settlement_micros = settlement_started.elapsed().as_micros();
    let structural_queue_micros =
        structural_resource_receipt.map_or(0, |receipt| receipt.queue_wait_micros);
    let structural_admitted_memory_bytes =
        structural_resource_receipt.map_or(0, |receipt| receipt.admitted_memory_bytes);
    eprintln!(
        "[runtime-search-stage-wall] key={request_id} retrievalMicros={retrieval_micros} retrievalQueueMicros={} retrievalAdmittedMemoryBytes={} ownerMaterializationMicros={owner_materialization_micros} structuralMicros={structural_micros} structuralQueueMicros={structural_queue_micros} structuralAdmittedMemoryBytes={structural_admitted_memory_bytes} graphMicros={graph_micros} resultMicros={result_micros} topologyMicros={topology_micros} settlementMicros={settlement_micros}",
        retrieval_resource_receipt.queue_wait_micros,
        retrieval_resource_receipt.admitted_memory_bytes,
    );
    drop(resource_permits);
    Ok(ProgressiveSearchProjection {
        result: settlement.as_json().clone(),
    })
}
