// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Graph fan-in and Project Topology settlement for progressive Search evidence.

use std::sync::Arc;

use crate::RuntimeQueryGeneration;
use crate::runtime_asp_client::AspClientOperationError;
use crate::runtime_asp_client::workspace_search_playbook::{
    ProgressiveSearchEvidence, bounded_graph_seed_scope,
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
        graph_seed_scope,
        graph_query_clauses,
        graph_relation_patterns,
        execution_budget,
        resident,
        topology_scope,
        lexical_wait_micros,
        retrieval_micros,
        owner_materialization_micros,
        projection_owner_count,
        projection_truncated,
        structural_micros,
        retrieval_resource_receipt,
        structural_resource_receipt,
        mut resource_permits,
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
            bounded_graph_seed_scope(&graph_seed_scope, graph_candidate_owner_limit);
        let graph_permit = generation
            .acquire_search_resources(
                agent_semantic_workspace_scheduler::RuntimeServerResourceRequest {
                    cpu: 1,
                    work_bytes: super::workspace_search_resources::scoped_source_work_bytes(
                        generation,
                        &topology_scope,
                    ),
                    memory_bytes: super::workspace_search_resources::graph_working_memory_bytes(
                        generation,
                        &topology_scope,
                    ),
                },
            )
            .await
            .map_err(AspClientOperationError::Message)?;
        let graph_resident = Arc::clone(&resident);
        let graph_scope = topology_scope.clone();
        let graph_budget = execution_budget.clone();
        let graph_request_id = request_id.to_owned();
        let graph_language_id = language_id.to_owned();
        let graph_clauses = graph_query_clauses.clone();
        let graph_patterns = graph_relation_patterns.clone();
        let graph_task = generation
            .spawn_search_blocking("runtime-search-graph-evaluation", move || {
                let mut graph_permit = graph_permit;
                let evaluated = (|| {
                    let graph_generation = graph_resident
                        .build_graph_generation_for_owner_scope(&graph_scope)
                        .map_err(AspClientOperationError::Message)?;
                    let evaluation_budget =
                        graph_budget.graph_evaluation_budget_for(&graph_generation);
                    let graph =
                        crate::runtime_search_graph::evaluate_resident_workspace_playbook_graph(
                            &graph_request_id,
                            &graph_language_id,
                            &graph_clauses,
                            &graph_patterns,
                            &candidate_owners,
                            evaluation_budget,
                            &graph_resident,
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
                        || evaluation_budget.is_some_and(|budget| {
                            candidate_owners.len() > budget.max_results
                                && graph.candidate_owner_ids.len() == budget.max_results
                        });
                    Ok::<_, AspClientOperationError>(
                        agent_semantic_search::WorkspaceSearchGraphFanIn {
                            ranked_candidate_owners: graph.candidate_owner_ids,
                            applied_clause_count: graph_clauses.len(),
                            complete: true,
                            truncated,
                        },
                    )
                })();
                graph_permit.release_cpu();
                evaluated.map(|fan_in| (fan_in, graph_permit))
            })
            .map_err(AspClientOperationError::Message)?;
        let (fan_in, graph_permit) = graph_task
            .join()
            .await
            .map_err(AspClientOperationError::Message)??;
        resource_permits.push(graph_permit);
        Some(fan_in)
    };
    let graph_micros = graph_started.elapsed().as_micros();
    let result_started = std::time::Instant::now();
    for receipt in &clause_receipts {
        eprintln!(
            "[runtime-search-clause] key={request_id} axis={} blockIndex={} priorityRank={} inputOwners={} outputOwners={} marginalOwnerReduction={} elapsedMicros={} complete={} coverageComplete={} truncated={}",
            receipt.axis.label(),
            receipt.block_index,
            receipt.priority_rank,
            receipt.input_owner_count,
            receipt.output_owner_count,
            receipt
                .marginal_owner_reduction
                .map_or_else(|| "na".to_owned(), |value| value.to_string()),
            receipt.elapsed_micros,
            receipt.complete,
            receipt.coverage_complete,
            receipt.truncated,
        );
    }
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
        .build_or_get_project_topology(project_root, &resident, &topology_scope)
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
        "[runtime-search-stage-wall] key={request_id} lexicalWaitMicros={lexical_wait_micros} retrievalMicros={retrieval_micros} retrievalQueueMicros={} retrievalAdmittedMemoryBytes={} projectionOwnerCount={projection_owner_count} projectionTruncated={projection_truncated} ownerMaterializationMicros={owner_materialization_micros} structuralMicros={structural_micros} structuralQueueMicros={structural_queue_micros} structuralAdmittedMemoryBytes={structural_admitted_memory_bytes} graphMicros={graph_micros} resultMicros={result_micros} topologyMicros={topology_micros} settlementMicros={settlement_micros}",
        retrieval_resource_receipt.queue_wait_micros,
        retrieval_resource_receipt.admitted_memory_bytes,
    );
    drop(resource_permits);
    Ok(ProgressiveSearchProjection {
        result: settlement.as_json().clone(),
    })
}
