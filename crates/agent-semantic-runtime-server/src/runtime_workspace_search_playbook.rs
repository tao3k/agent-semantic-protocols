// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Runtime execution of the Agent-authored progressive Search Playbook.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, LazyLock};

use crate::RuntimeQueryGeneration;
use crate::runtime_asp_client::AspClientOperationError;
use crate::runtime_asp_client::syntax_query_route::execute_workspace_syntax_query_evidence;
use crate::runtime_resident_grep::{RuntimeGrepMatch, execute_runtime_resident_grep_blocks};
use agent_semantic_search::{
    GraphNativeBlock, SearchPlaybookClauseAxis, WorkspaceSearchAxisKind,
    WorkspaceSearchClauseReceipt, WorkspaceSearchPlaybookPlan, WorkspaceSearchSyntaxCandidate,
};

static RESIDENT_SEARCH_CPU_LANES: LazyLock<Arc<tokio::sync::Semaphore>> = LazyLock::new(|| {
    let lanes = std::thread::available_parallelism()
        .map_or(1, std::num::NonZeroUsize::get)
        .clamp(1, 8);
    Arc::new(tokio::sync::Semaphore::new(lanes))
});

pub(super) struct ProgressiveSearchEvidence {
    pub(super) clause_receipts: Vec<WorkspaceSearchClauseReceipt>,
    pub(super) syntax_candidates: Vec<WorkspaceSearchSyntaxCandidate>,
    pub(super) graph_query_clauses: Vec<GraphNativeBlock>,
    pub(super) graph_relation_patterns: Vec<agent_semantic_search::ResidentGraphRelationPattern>,
    pub(super) execution_budget:
        crate::runtime_search_execution_budget::RuntimeSearchExecutionBudget,
    pub(super) resident: agent_semantic_client_db::runtime_resident_read::RuntimeResidentReadClient,
    pub(super) topology_scope: BTreeSet<String>,
}

pub(super) struct ProgressiveSearchProjection {
    pub(super) result: serde_json::Value,
}

struct SearchClauseExecution {
    priority_rank: usize,
    receipt: WorkspaceSearchClauseReceipt,
    syntax_candidates: Vec<WorkspaceSearchSyntaxCandidate>,
}

struct RetrievalLayoutExecution {
    clauses: Vec<SearchClauseExecution>,
    fused_scope: BTreeSet<String>,
    fused_matches: Vec<RuntimeGrepMatch>,
    tantivy_expressions_by_owner: BTreeMap<String, BTreeSet<String>>,
}

pub(super) async fn execute_progressive_search_clauses(
    plan: &WorkspaceSearchPlaybookPlan,
    project_root: &std::path::Path,
    generation: Arc<RuntimeQueryGeneration>,
    providers: &[agent_semantic_search::WorkspaceSearchProvider],
    request_id: &str,
    workspace_identity: &str,
    parser_artifact_root: &std::path::Path,
    runtime_search_service: &agent_semantic_client_db::runtime_search_service::RuntimeSearchServiceHandle,
    owner_materializer: &super::owner_materialization::RuntimeOwnerMaterializer,
    workspace_registry: &Arc<
        agent_semantic_client_db::runtime_server_workspace::RuntimeServerWorkspaceRegistry,
    >,
) -> Result<ProgressiveSearchEvidence, AspClientOperationError> {
    if !plan.axes.tantivy.is_empty() {
        generation
            .await_lexical_attachment()
            .await
            .map_err(|message| {
                AspClientOperationError::Terminal(
                    agent_semantic_client_server::AspClientDispatchError {
                        reason_kind: "runtime-search-lexical-attachment-terminal-missing"
                            .to_owned(),
                        message,
                        details: Some(serde_json::json!({
                            "failureStage": "runtime-search-lexical-admission",
                            "runtimeGenerationDigest": generation.generation_digest(),
                            "terminalCount": 1
                        })),
                    },
                )
            })?;
    }
    let execution_budget =
        crate::runtime_search_execution_budget::RuntimeSearchExecutionBudget::derive(
            generation.as_ref(),
        )
        .map_err(AspClientOperationError::Message)?;
    let mut input_rank = 0;
    let mut graph_query_clauses = Vec::new();
    let mut graph_relation_patterns = Vec::new();
    let mut retrieval_clauses = Vec::new();
    let mut structural_clauses = Vec::new();
    for clause in &plan.axes.clause_order {
        if clause.axis == SearchPlaybookClauseAxis::Graph {
            let block = plan.axes.graph.get(clause.block_index).ok_or_else(|| {
                AspClientOperationError::Message("Graph clause index is out of bounds".to_owned())
            })?;
            graph_relation_patterns.push(compile_graph_relation_pattern(block)?);
            graph_query_clauses.push(block.clone());
            continue;
        }

        let priority_rank = input_rank;
        if matches!(
            clause.axis,
            SearchPlaybookClauseAxis::Rg | SearchPlaybookClauseAxis::Tantivy
        ) {
            retrieval_clauses.push((clause.axis, clause.block_index, priority_rank));
            input_rank += 1;
            continue;
        }
        structural_clauses.push((clause.axis, clause.block_index, priority_rank));
        input_rank += 1;
    }

    // The admitted layout, rather than CLI spelling order, owns execution:
    // rg and Tantivy first determine one fused file-context scope; explicit
    // syntax/native-syntax queries then determine the structural frontier
    // inside it.
    let cpu_permit = Arc::clone(&RESIDENT_SEARCH_CPU_LANES)
        .acquire_owned()
        .await
        .map_err(|_| {
            AspClientOperationError::Message("resident Search CPU lanes closed".to_owned())
        })?;
    let retrieval_plan = plan.clone();
    let retrieval_generation = Arc::clone(&generation);
    let retrieval_budget = execution_budget.clone();
    let mut retrieval = tokio::task::spawn_blocking(move || {
        let _cpu_permit = cpu_permit;
        execute_default_retrieval_layout(
            &retrieval_plan,
            retrieval_generation.as_ref(),
            &retrieval_budget,
            &retrieval_clauses,
        )
    })
    .await
    .map_err(|error| {
        AspClientOperationError::Message(format!("resident Search CPU lane failed: {error}"))
    })??;
    let mut resident = owner_materializer
        .ensure_candidates(
            request_id,
            workspace_identity,
            project_root,
            parser_artifact_root,
            generation.generation_digest(),
            &retrieval.fused_scope,
            providers,
            runtime_search_service,
            workspace_registry,
        )
        .await?;
    let topology_scope = if graph_query_clauses.is_empty() {
        retrieval.fused_scope.clone()
    } else {
        relation_neighbor_scope(
            &resident,
            &retrieval.fused_scope,
            execution_budget
                .graph_candidate_owner_limit()
                .max(retrieval.fused_scope.len()),
        )?
    };
    if topology_scope != retrieval.fused_scope {
        resident = owner_materializer
            .ensure_candidates(
                request_id,
                workspace_identity,
                project_root,
                parser_artifact_root,
                generation.generation_digest(),
                &topology_scope,
                providers,
                runtime_search_service,
                workspace_registry,
            )
            .await?;
    }
    let grounding_permit = Arc::clone(&RESIDENT_SEARCH_CPU_LANES)
        .acquire_owned()
        .await
        .map_err(|_| {
            AspClientOperationError::Message("resident Search CPU lanes closed".to_owned())
        })?;
    let grounding_scope = retrieval.fused_scope.clone();
    let grounding_matches = retrieval.fused_matches.clone();
    let grounding_resident = workspace_registry
        .resident_read_client(workspace_identity, project_root)
        .map_err(AspClientOperationError::Message)?;
    let mut syntax_candidates = tokio::task::spawn_blocking(move || {
        let _cpu_permit = grounding_permit;
        syntax_candidates_enclosing_rg_matches(
            &grounding_resident,
            &grounding_scope,
            grounding_matches.iter(),
        )
    })
    .await
    .map_err(|error| {
        AspClientOperationError::Message(format!("resident parser grounding lane failed: {error}"))
    })??;
    for candidate in &mut syntax_candidates {
        candidate.hit.tantivy = retrieval
            .tantivy_expressions_by_owner
            .get(&candidate.owner)
            .into_iter()
            .flatten()
            .cloned()
            .collect();
        if !candidate.hit.rg.is_empty() {
            candidate.relation = "native-parser:rg-tantivy-fused".to_owned();
        }
    }
    if let Some(clause) = retrieval
        .clauses
        .iter_mut()
        .find(|clause| clause.receipt.axis == WorkspaceSearchAxisKind::Rg)
    {
        clause.syntax_candidates = syntax_candidates;
    }
    if !structural_clauses.is_empty() {
        for clause in &mut retrieval.clauses {
            // An explicit structural query owns the structural frontier. The
            // automatic grounding result is the default only when no explicit
            // syntax or native-syntax block was supplied.
            clause.syntax_candidates.clear();
        }
    }
    let mut clause_executions = retrieval.clauses;
    for (axis, block_index, priority_rank) in structural_clauses {
        let mut syntax_candidates = Vec::new();
        let receipt = match axis {
            SearchPlaybookClauseAxis::Rg | SearchPlaybookClauseAxis::Tantivy => {
                unreachable!("retrieval inputs execute through the admitted shared-scope layout")
            }
            SearchPlaybookClauseAxis::Syntax => {
                let block = plan.axes.syntax.get(block_index).ok_or_else(|| {
                    AspClientOperationError::Message(
                        "syntax clause index is out of bounds".to_owned(),
                    )
                })?;
                let syntax_request =
                    agent_semantic_client_protocol::AspClientWorkspaceSyntaxQueryRequest {
                        schema_id:
                            "agent.semantic-protocols.asp-client-workspace-syntax-query-request"
                                .to_owned(),
                        schema_version: "1".to_owned(),
                        languages: plan.language.clone(),
                        documents: plan.documents.clone(),
                        workspace: None,
                        syntax: vec![block.clone()],
                        projection: "matches".to_owned(),
                    };
                let structural_resident = workspace_registry
                    .resident_read_client(workspace_identity, project_root)
                    .map_err(AspClientOperationError::Message)?;
                let evidence = execute_workspace_syntax_query_evidence(
                    &syntax_request,
                    generation.generation_digest(),
                    &structural_resident,
                    providers,
                    execution_budget.syntax_selector_limit(),
                    Some(&retrieval.fused_scope),
                )
                .await?;
                let owners = evidence
                    .iter()
                    .map(|item| item.owner.clone())
                    .collect::<BTreeSet<_>>()
                    .into_iter()
                    .collect();
                syntax_candidates.extend(evidence.iter().map(|item| {
                    WorkspaceSearchSyntaxCandidate {
                        owner: item.owner.clone(),
                        selector: item.selector.clone(),
                        relation: item.relation.clone(),
                        hit: agent_semantic_search::WorkspaceSearchHitProjection {
                            native: true,
                            ..Default::default()
                        },
                    }
                }));
                complete_receipt(
                    WorkspaceSearchAxisKind::Syntax,
                    block_index,
                    priority_rank,
                    owners,
                    evidence.len() < execution_budget.syntax_selector_limit(),
                    evidence.len() == execution_budget.syntax_selector_limit(),
                )
            }
            SearchPlaybookClauseAxis::NativeSyntax => {
                let selector = plan.axes.native_syntax.get(block_index).ok_or_else(|| {
                    AspClientOperationError::Message(
                        "native-syntax clause index is out of bounds".to_owned(),
                    )
                })?;
                let canonical = agent_semantic_content_identity::CanonicalItemSelector::parse_root_or_exact_descendant(selector)
                    .map_err(|error| {
                        AspClientOperationError::Message(format!(
                            "native-syntax selector is not canonical: {error}"
                        ))
                    })?;
                let owner = canonical
                    .owner_path()
                    .map_err(AspClientOperationError::Message)?;
                if !retrieval.fused_scope.contains(&owner) {
                    return Err(AspClientOperationError::Message(format!(
                        "native-syntax selector owner is outside the fused rg/Tantivy file-context scope: {owner}"
                    )));
                }
                let resident = workspace_registry
                    .resident_read_client(workspace_identity, project_root)
                    .map_err(AspClientOperationError::Message)?;
                let (projections, _, diagnostics) = resident
                    .native_syntax_playbook_projection(std::slice::from_ref(&owner))
                    .map_err(AspClientOperationError::Message)?;
                if let Some(diagnostic) = diagnostics.first() {
                    return Err(AspClientOperationError::Message(format!(
                        "native-syntax selector owner is unavailable: reasonKind={} owner={}",
                        diagnostic.reason_kind, diagnostic.owner_path
                    )));
                }
                let admitted = projections.iter().any(|projection| {
                    projection
                        .selectors
                        .iter()
                        .any(|candidate| candidate.selector == *selector)
                });
                if !admitted {
                    return Err(AspClientOperationError::Message(format!(
                        "native-syntax selector is not present in the admitted generation: {selector}"
                    )));
                }
                syntax_candidates.push(WorkspaceSearchSyntaxCandidate {
                    owner: owner.clone(),
                    selector: selector.clone(),
                    relation: "native-syntax-selector".to_owned(),
                    hit: agent_semantic_search::WorkspaceSearchHitProjection {
                        native: true,
                        ..Default::default()
                    },
                });
                complete_receipt(
                    WorkspaceSearchAxisKind::NativeSyntax,
                    block_index,
                    priority_rank,
                    vec![owner],
                    true,
                    false,
                )
            }
            SearchPlaybookClauseAxis::Graph => unreachable!("Graph is a fan-in barrier"),
        };
        clause_executions.push(SearchClauseExecution {
            priority_rank,
            receipt,
            syntax_candidates,
        });
    }
    clause_executions.sort_by_key(|execution| execution.priority_rank);
    let mut clause_receipts = Vec::with_capacity(clause_executions.len());
    let mut syntax_candidates = Vec::new();
    for execution in clause_executions {
        clause_receipts.push(execution.receipt);
        syntax_candidates.extend(execution.syntax_candidates);
    }

    Ok(ProgressiveSearchEvidence {
        clause_receipts,
        syntax_candidates,
        graph_query_clauses,
        graph_relation_patterns,
        execution_budget,
        resident,
        topology_scope,
    })
}

pub(super) fn relation_neighbor_scope(
    resident: &agent_semantic_client_db::runtime_resident_read::RuntimeResidentReadClient,
    seed: &BTreeSet<String>,
    limit: usize,
) -> Result<BTreeSet<String>, AspClientOperationError> {
    let mut scope = seed.clone();
    if scope.len() >= limit {
        return Ok(scope);
    }
    let indexed = resident
        .indexed_owner_paths()
        .into_iter()
        .collect::<BTreeSet<_>>();
    for segment in resident
        .topology_source_segments()
        .map_err(AspClientOperationError::Message)?
        .into_iter()
        .filter(|segment| seed.contains(&segment.owner_path))
    {
        for relation in segment.relations {
            for endpoint in [&relation.relation.from, &relation.relation.to] {
                let owner = match endpoint.kind {
                    agent_semantic_content_identity::ProviderRelationEndpointKindV1::Owner => {
                        Some(endpoint.id.clone())
                    }
                    agent_semantic_content_identity::ProviderRelationEndpointKindV1::Item => {
                        agent_semantic_content_identity::CanonicalItemSelector::parse(
                            endpoint.id.clone(),
                        )
                        .ok()
                        .and_then(|selector| selector.owner_path().ok())
                    }
                };
                if let Some(owner) = owner
                    && indexed.contains(&owner)
                {
                    scope.insert(owner);
                    if scope.len() == limit {
                        return Ok(scope);
                    }
                }
            }
        }
    }
    Ok(scope)
}

fn execute_default_retrieval_layout(
    plan: &WorkspaceSearchPlaybookPlan,
    generation: &RuntimeQueryGeneration,
    budget: &crate::runtime_search_execution_budget::RuntimeSearchExecutionBudget,
    clauses: &[(SearchPlaybookClauseAxis, usize, usize)],
) -> Result<RetrievalLayoutExecution, AspClientOperationError> {
    if clauses.is_empty() {
        return Ok(RetrievalLayoutExecution {
            clauses: Vec::new(),
            fused_scope: BTreeSet::new(),
            fused_matches: Vec::new(),
            tantivy_expressions_by_owner: BTreeMap::new(),
        });
    }
    let rg_clauses = clauses
        .iter()
        .copied()
        .filter(|(axis, _, _)| *axis == SearchPlaybookClauseAxis::Rg)
        .collect::<Vec<_>>();
    let tantivy_clauses = clauses
        .iter()
        .copied()
        .filter(|(axis, _, _)| *axis == SearchPlaybookClauseAxis::Tantivy)
        .collect::<Vec<_>>();
    if rg_clauses.is_empty() || tantivy_clauses.is_empty() {
        return Err(AspClientOperationError::Message(
            "Search Layout requires rg and Tantivy shared-scope inputs together".to_owned(),
        ));
    }

    // Tantivy is evaluated independently first. Besides ranked recall, its
    // bounded owner set is the sound fused scope for V1 regexes that cannot
    // produce mandatory trigrams (short patterns and Unicode case folding).
    let mut tantivy_results = Vec::with_capacity(tantivy_clauses.len());
    let mut tantivy_scope = BTreeSet::new();
    for (_, block_index, priority_rank) in tantivy_clauses {
        let block = plan.axes.tantivy.get(block_index).ok_or_else(|| {
            AspClientOperationError::Message("Tantivy clause index is out of bounds".to_owned())
        })?;
        let result = execute_tantivy_block(
            block,
            &plan.routes,
            generation,
            budget.lexical_owner_limit(),
        )?;
        result.require_complete_fused_scope()?;
        tantivy_scope.extend(result.owners.iter().cloned());
        tantivy_results.push((block_index, priority_rank, block.join(" "), result));
    }

    let mut rg_results = Vec::with_capacity(rg_clauses.len());
    let mut rg_scope = BTreeSet::new();
    for (_, block_index, priority_rank) in rg_clauses {
        let block = plan.axes.rg.get(block_index).ok_or_else(|| {
            AspClientOperationError::Message("rg clause index is out of bounds".to_owned())
        })?;
        let result = execute_runtime_resident_grep_blocks(
            generation.resident().resident_grep_corpus(),
            std::slice::from_ref(block),
            budget.rg_match_limit(),
            |candidate_plan, limit| {
                resident_grep_candidate_scope(candidate_plan, &tantivy_scope, limit, || {
                    generation
                        .resident()
                        .resident_grep_candidate_owner_paths(candidate_plan, limit)
                })
            },
        )
        .map_err(|message| {
            let reason_kind = message
                .strip_prefix("reasonKind=")
                .and_then(|suffix| suffix.split_whitespace().next());
            if reason_kind.is_some_and(|kind| kind.starts_with("resident-rg-")) {
                AspClientOperationError::Terminal(
                    agent_semantic_client_server::AspClientDispatchError {
                        reason_kind: reason_kind.expect("checked resident rg reason").to_owned(),
                        message,
                        details: Some(serde_json::json!({
                            "failureStage": "resident-rg-admission",
                            "runtimeGenerationDigest": generation.generation_digest(),
                            "terminalCount": 1
                        })),
                    },
                )
            } else {
                AspClientOperationError::Message(message)
            }
        })?;
        rg_scope.extend(result.candidate_owner_paths.iter().cloned());
        rg_results.push((block_index, priority_rank, result));
    }

    let fused_scope = fused_file_context_scope(&rg_scope, &tantivy_scope);
    let fused_matches = rg_results
        .iter()
        .flat_map(|(_, _, result)| result.grounding_matches.iter())
        .filter(|matched| fused_scope.contains(&matched.owner_path))
        .cloned()
        .collect::<Vec<_>>();
    let mut tantivy_expressions_by_owner = BTreeMap::<String, BTreeSet<String>>::new();
    for (_, _, expression, result) in &tantivy_results {
        for owner in &result.owners {
            tantivy_expressions_by_owner
                .entry(owner.clone())
                .or_default()
                .insert(expression.clone());
        }
    }
    let mut executions = Vec::with_capacity(rg_results.len() + tantivy_results.len());
    for (block_index, priority_rank, result) in rg_results {
        let candidate_owners = result
            .candidate_owner_paths
            .into_iter()
            .filter(|owner| fused_scope.contains(owner))
            .collect();
        executions.push(SearchClauseExecution {
            priority_rank,
            receipt: complete_receipt(
                WorkspaceSearchAxisKind::Rg,
                block_index,
                priority_rank,
                candidate_owners,
                !result.truncated,
                result.truncated,
            ),
            syntax_candidates: Vec::new(),
        });
    }
    for (block_index, priority_rank, _, result) in tantivy_results {
        executions.push(SearchClauseExecution {
            priority_rank,
            receipt: complete_receipt(
                WorkspaceSearchAxisKind::Tantivy,
                block_index,
                priority_rank,
                result.owners,
                !result.truncated,
                result.truncated,
            ),
            syntax_candidates: Vec::new(),
        });
    }
    Ok(RetrievalLayoutExecution {
        clauses: executions,
        fused_scope,
        fused_matches,
        tantivy_expressions_by_owner,
    })
}

fn resident_grep_candidate_scope(
    candidate_plan: &agent_semantic_search::ResidentGrepCandidatePlan,
    tantivy_scope: &BTreeSet<String>,
    limit: usize,
    trigram_candidates: impl FnOnce() -> Result<
        (
            Vec<String>,
            agent_semantic_search::ResidentByteCoverageQueryReceipt,
        ),
        String,
    >,
) -> Result<
    (
        Vec<String>,
        agent_semantic_search::ResidentByteCoverageQueryReceipt,
    ),
    String,
> {
    if !candidate_plan.is_match_all() {
        let (mut owners, mut receipt) = trigram_candidates()?;
        owners.retain(|owner| tantivy_scope.contains(owner));
        receipt.candidate_count = owners.len();
        return Ok((owners, receipt));
    }
    if tantivy_scope.len() > limit {
        return Err(format!(
            "query-not-ready: resident GREP fused candidate budget exceeded: candidates={} limit={limit}",
            tantivy_scope.len()
        ));
    }
    Ok((
        tantivy_scope.iter().cloned().collect(),
        agent_semantic_search::ResidentByteCoverageQueryReceipt {
            requested_gram_count: 0,
            decoded_posting_count: 0,
            smallest_posting_count: 0,
            candidate_count: tantivy_scope.len(),
            lookup_nanos: 0,
        },
    ))
}

fn fused_file_context_scope(
    rg_scope: &BTreeSet<String>,
    tantivy_scope: &BTreeSet<String>,
) -> BTreeSet<String> {
    rg_scope.intersection(tantivy_scope).cloned().collect()
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
    } = evidence;
    execution_budget
        .validate_for_generation(generation.generation_digest())
        .map_err(AspClientOperationError::Message)?;
    let graph_candidate_owner_limit = execution_budget.graph_candidate_owner_limit();
    let evidence_item_limit = execution_budget.evidence_item_limit();
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
    let workspace_result = agent_semantic_search::synthesize_workspace_search_playbook_result(
        clause_receipts,
        syntax_candidates,
        graph_fan_in,
        evidence_item_limit,
    )?;
    let workspace_result = serde_json::to_value(workspace_result)
        .map_err(|error| AspClientOperationError::Message(error.to_string()))?;
    let attachment = generation
        .build_project_topology_for_owner_scope(project_root, &resident, &topology_scope)
        .await
        .map_err(AspClientOperationError::Message)?;
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
    Ok(ProgressiveSearchProjection {
        result: settlement.as_json().clone(),
    })
}

fn structural_candidate_owner_scope(
    candidates: &[WorkspaceSearchSyntaxCandidate],
    limit: usize,
) -> (Vec<String>, bool) {
    let mut seen = BTreeSet::new();
    let mut owners = Vec::new();
    for candidate in candidates {
        if seen.insert(candidate.owner.as_str()) {
            if owners.len() == limit {
                return (owners, true);
            }
            owners.push(candidate.owner.clone());
        }
    }
    (owners, false)
}

fn syntax_candidates_enclosing_rg_matches<'a>(
    resident: &agent_semantic_client_db::runtime_resident_read::RuntimeResidentReadClient,
    owner_paths: &BTreeSet<String>,
    matches: impl IntoIterator<Item = &'a RuntimeGrepMatch>,
) -> Result<Vec<WorkspaceSearchSyntaxCandidate>, AspClientOperationError> {
    let matches = matches.into_iter().cloned().collect::<Vec<_>>();
    let owners = owner_paths.iter().cloned().collect::<Vec<_>>();
    if owners.is_empty() {
        return Ok(Vec::new());
    }
    let (projections, _, _) = resident
        .native_syntax_playbook_projection(&owners)
        .map_err(AspClientOperationError::Message)?;
    let projections = projections
        .into_iter()
        .map(|projection| (projection.owner_path.clone(), projection))
        .collect::<BTreeMap<_, _>>();
    let mut line_ranges = BTreeMap::new();
    for owner in &owners {
        let snapshot = resident
            .owner_snapshot(owner)
            .map_err(AspClientOperationError::Message)?
            .ok_or_else(|| {
                AspClientOperationError::Message(format!(
                    "rg syntax mapping owner disappeared: {owner}"
                ))
            })?;
        line_ranges.insert(owner.clone(), source_line_ranges(&snapshot.bytes));
    }

    let matched_owners = matches
        .iter()
        .map(|item| item.owner_path.clone())
        .collect::<BTreeSet<_>>();
    let mut candidates = BTreeMap::<(String, String), WorkspaceSearchSyntaxCandidate>::new();
    for owner in owner_paths.difference(&matched_owners) {
        let Some(projection) = projections.get(owner) else {
            continue;
        };
        for selector in &projection.selectors {
            candidates.insert(
                (owner.clone(), selector.selector.clone()),
                WorkspaceSearchSyntaxCandidate {
                    owner: owner.clone(),
                    selector: selector.selector.clone(),
                    relation: "native-parser:rg-owner-scope".to_owned(),
                    hit: agent_semantic_search::WorkspaceSearchHitProjection {
                        native: true,
                        ..Default::default()
                    },
                },
            );
        }
    }
    for item in matches {
        let Some((line_start, line_end)) = line_ranges
            .get(&item.owner_path)
            .and_then(|ranges| {
                usize::try_from(item.owner_line)
                    .ok()?
                    .checked_sub(1)
                    .and_then(|line| ranges.get(line))
            })
            .copied()
        else {
            return Err(AspClientOperationError::Message(format!(
                "rg syntax mapping line is outside owner: owner={} line={}",
                item.owner_path, item.owner_line
            )));
        };
        let Some(projection) = projections.get(&item.owner_path) else {
            continue;
        };
        let enclosing = smallest_selector_overlapping_line(projection, line_start, line_end);
        if let Some(selector) = enclosing {
            let candidate = candidates
                .entry((item.owner_path.clone(), selector.selector.clone()))
                .or_insert_with(|| WorkspaceSearchSyntaxCandidate {
                    owner: item.owner_path.clone(),
                    selector: selector.selector.clone(),
                    relation: "syntax-encloses:rg-match".to_owned(),
                    hit: agent_semantic_search::WorkspaceSearchHitProjection {
                        native: true,
                        ..Default::default()
                    },
                });
            candidate.hit.rg.push([item.owner_line, item.owner_line]);
        }
    }
    for candidate in candidates.values_mut() {
        candidate.hit.rg.sort_unstable();
        candidate.hit.rg.dedup();
    }
    Ok(candidates.into_values().collect())
}

fn smallest_selector_overlapping_line(
    projection: &agent_semantic_search::NativeSyntaxProjection,
    line_start: usize,
    line_end: usize,
) -> Option<&agent_semantic_search::NativeSyntaxSelector> {
    projection
        .selectors
        .iter()
        .filter(|selector| selector.byte_start < line_end && line_start < selector.byte_end)
        .min_by(|left, right| {
            (left.byte_end - left.byte_start)
                .cmp(&(right.byte_end - right.byte_start))
                .then_with(|| left.selector.cmp(&right.selector))
        })
}

fn source_line_ranges(bytes: &[u8]) -> Vec<(usize, usize)> {
    let mut starts = vec![0];
    starts.extend(
        bytes
            .iter()
            .enumerate()
            .filter(|(_, byte)| **byte == b'\n')
            .map(|(index, _)| index + 1)
            .filter(|offset| *offset < bytes.len()),
    );
    starts
        .iter()
        .enumerate()
        .map(|(index, start)| {
            (
                *start,
                starts.get(index + 1).copied().unwrap_or(bytes.len()),
            )
        })
        .collect()
}

fn compile_graph_relation_pattern(
    block: &GraphNativeBlock,
) -> Result<agent_semantic_search::ResidentGraphRelationPattern, AspClientOperationError> {
    agent_semantic_mrr::compile_graph_relation_pattern_v1(
        &block.language,
        &block.argv,
        Some("Owner"),
    )
    .map_err(|error| AspClientOperationError::Message(error.to_string()))
}

struct TantivyClauseResult {
    owners: Vec<String>,
    truncated: bool,
}

impl TantivyClauseResult {
    fn require_complete_fused_scope(&self) -> Result<(), AspClientOperationError> {
        if self.truncated {
            return Err(AspClientOperationError::Message(
                "query-not-ready: Tantivy candidate scope truncated before GREP intersection"
                    .to_owned(),
            ));
        }
        Ok(())
    }
}

fn execute_tantivy_block(
    block: &[String],
    routes: &[agent_semantic_search::WorkspaceSearchPlaybookRoute],
    generation: &RuntimeQueryGeneration,
    limit: u32,
) -> Result<TantivyClauseResult, AspClientOperationError> {
    let mut owners = Vec::new();
    let mut seen = BTreeSet::new();
    let mut truncated = false;
    // Tantivy's query parser consumes one expression string. Preserve the
    // native argv block until this adapter; `|` remains query syntax and is
    // never reinterpreted as an ASP-private fan-out separator.
    let expression = block.join(" ");
    let analysis = agent_semantic_search::analyze_tantivy_query(&expression);
    if !analysis.is_admitted() {
        return Err(AspClientOperationError::Message(format!(
            "search-playbook-tantivy-expression-not-admitted: syntaxDiagnostics={:?} unsupportedFields={:?} missingFeatures={:?}",
            analysis.syntax_diagnostics, analysis.unsupported_fields, analysis.missing_features,
        )));
    }
    let generation_owner_limit = u32::try_from(generation.resident().indexed_owner_count())
        .unwrap_or(u32::MAX)
        .max(1);
    for route in routes {
        let language = agent_semantic_client_core::LanguageId::try_from(route.language_id.as_str())
            .map_err(|error| AspClientOperationError::Message(error.to_string()))?;
        let result = generation
            .resident()
            .read_tantivy_for_language(&expression, &language, generation_owner_limit)
            .map_err(AspClientOperationError::Message)?;
        for hit in &result.hits {
            if generation
                .resident()
                .contains_indexed_owner(&hit.owner_path)
                && seen.insert(hit.owner_path.clone())
            {
                if owners.len() == limit as usize {
                    truncated = true;
                    break;
                }
                owners.push(hit.owner_path.clone());
            }
        }
    }
    Ok(TantivyClauseResult { owners, truncated })
}

fn complete_receipt(
    axis: WorkspaceSearchAxisKind,
    block_index: usize,
    priority_rank: usize,
    candidate_owners: Vec<String>,
    coverage_complete: bool,
    truncated: bool,
) -> WorkspaceSearchClauseReceipt {
    WorkspaceSearchClauseReceipt {
        axis,
        block_index,
        priority_rank,
        candidate_owners,
        complete: true,
        coverage_complete,
        truncated,
    }
}

#[cfg(test)]
#[path = "../tests/unit/runtime_workspace_search_playbook.rs"]
mod tests;
