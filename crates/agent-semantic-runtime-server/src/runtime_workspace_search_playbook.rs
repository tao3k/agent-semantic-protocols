// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Runtime execution of the Agent-authored progressive Search Playbook.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

#[cfg(test)]
use super::workspace_search_lexical::TantivyClauseResult;
use super::workspace_search_lexical::{
    SearchClauseMetrics, branch_marginal_reductions, complete_receipt, elapsed_micros,
    execute_tantivy_block, require_complete_intersection_branch, resident_grep_candidate_scope,
};
use super::workspace_search_topology::execute_topology_block;
#[cfg(test)]
use super::workspace_search_topology::topology_owner_language;
use crate::RuntimeQueryGeneration;
use crate::runtime_asp_client::AspClientOperationError;
use crate::runtime_asp_client::syntax_query_route::execute_workspace_syntax_query_evidence;
use crate::runtime_search_execution::{RuntimeGrepMatch, execute_runtime_resident_grep_blocks};
use agent_semantic_search::{
    GraphNativeBlock, SearchPlaybookClauseAxis, WorkspaceSearchAxisKind,
    WorkspaceSearchClauseReceipt, WorkspaceSearchPlaybookPlan, WorkspaceSearchSyntaxCandidate,
};

#[path = "runtime_workspace_search_structural_scope.rs"]
mod structural_scope;
pub(super) use structural_scope::{
    bounded_graph_seed_scope, bounded_semantic_projection_scope, compile_graph_relation_pattern,
    intersect_clause_owner_scopes, relation_neighbor_scope, resident_semantic_scope,
    syntax_candidates_enclosing_rg_matches,
};
#[cfg(test)]
use structural_scope::{
    fused_file_context_scope, smallest_selector_overlapping_line, source_line_ranges,
};

pub(super) struct ProgressiveSearchEvidence {
    pub(super) clause_receipts: Vec<WorkspaceSearchClauseReceipt>,
    pub(super) syntax_candidates: Vec<WorkspaceSearchSyntaxCandidate>,
    pub(super) graph_seed_scope: BTreeSet<String>,
    pub(super) graph_query_clauses: Vec<GraphNativeBlock>,
    pub(super) graph_relation_patterns: Vec<agent_semantic_search::ResidentGraphRelationPattern>,
    pub(super) execution_budget:
        crate::runtime_search_execution_budget::RuntimeSearchExecutionBudget,
    pub(super) resident:
        Arc<agent_semantic_client_db::runtime_resident_read::RuntimeResidentReadClient>,
    pub(super) topology_scope: BTreeSet<String>,
    pub(super) lexical_wait_micros: u128,
    pub(super) retrieval_micros: u128,
    pub(super) owner_materialization_micros: u128,
    pub(super) projection_owner_count: usize,
    pub(super) projection_truncated: bool,
    pub(super) structural_micros: u128,
    pub(super) retrieval_resource_receipt:
        agent_semantic_workspace_scheduler::RuntimeServerResourcePermitReceipt,
    pub(super) structural_resource_receipt:
        Option<agent_semantic_workspace_scheduler::RuntimeServerResourcePermitReceipt>,
    pub(super) resource_permits:
        Vec<agent_semantic_workspace_scheduler::RuntimeServerResourcePermit>,
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum RetrievalCompositionKind {
    None,
    Single,
    Intersect,
}

fn retrieval_composition_kind(
    composition: &agent_semantic_client_protocol::AspClientSearchPlaybookComposition,
) -> RetrievalCompositionKind {
    use agent_semantic_client_protocol::AspClientSearchPlaybookClauseAxis as Axis;
    use agent_semantic_client_protocol::AspClientSearchPlaybookComposition as Composition;

    match composition {
        Composition::Leaf { clause }
            if matches!(clause.axis, Axis::Rg | Axis::Tantivy | Axis::Topology) =>
        {
            RetrievalCompositionKind::Single
        }
        Composition::Leaf { .. } => RetrievalCompositionKind::None,
        Composition::Intersect { .. } => RetrievalCompositionKind::Intersect,
        Composition::Chain { children } => children
            .first()
            .map_or(RetrievalCompositionKind::None, retrieval_composition_kind),
    }
}

#[expect(
    clippy::too_many_arguments,
    reason = "SearchLoop execution keeps V1 identity, route, budget, and telemetry inputs explicit"
)]
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
    let lexical_wait_started = std::time::Instant::now();
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
    let lexical_wait_micros = lexical_wait_started.elapsed().as_micros();
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
            SearchPlaybookClauseAxis::Rg
                | SearchPlaybookClauseAxis::Tantivy
                | SearchPlaybookClauseAxis::Topology
        ) {
            retrieval_clauses.push((clause.axis, clause.block_index, priority_rank));
            input_rank += 1;
            continue;
        }
        structural_clauses.push((clause.axis, clause.block_index, priority_rank));
        input_rank += 1;
    }

    // The admitted typed route, rather than engine availability, owns
    // execution. Regex and ranked-text leaves run independently unless an
    // explicit intersection includes both; structural-only routes begin from
    // the immutable Workspace universe and do no lexical acquisition.
    let retrieval_started = std::time::Instant::now();
    let retrieval_permit = generation
        .acquire_search_resources(
            agent_semantic_workspace_scheduler::RuntimeServerResourceRequest {
                cpu: 1,
                work_bytes: super::workspace_search_resources::retrieval_work_bytes(
                    generation.as_ref(),
                ),
                memory_bytes: super::workspace_search_resources::retrieval_working_memory_bytes(
                    generation.as_ref(),
                ),
            },
        )
        .await
        .map_err(AspClientOperationError::Message)?;
    let retrieval_resource_receipt = retrieval_permit.receipt();
    let has_retrieval_clauses = !retrieval_clauses.is_empty();
    let retrieval_plan = plan.clone();
    let retrieval_generation = Arc::clone(&generation);
    let retrieval_budget = execution_budget.clone();
    let retrieval_task = generation
        .spawn_search_blocking("runtime-search-retrieval", move || {
            execute_default_retrieval_layout(
                &retrieval_plan,
                retrieval_generation.as_ref(),
                &retrieval_budget,
                &retrieval_clauses,
            )
            .map(|retrieval| (retrieval, retrieval_permit))
        })
        .map_err(AspClientOperationError::Message)?;
    let (mut retrieval, mut retrieval_permit) =
        retrieval_task.join().await.map_err(|error| {
            AspClientOperationError::Message(format!("resident Search CPU lane failed: {error}"))
        })??;
    retrieval_permit.release_cpu();
    let retrieval_micros = retrieval_started.elapsed().as_micros();
    let ranked_retrieval_branches = retrieval
        .clauses
        .iter()
        .map(|execution| execution.receipt.candidate_owners.as_slice())
        .collect::<Vec<_>>();
    let (projection_scope, projection_truncated) =
        if has_retrieval_clauses && structural_clauses.is_empty() {
            bounded_semantic_projection_scope(
                &retrieval.fused_scope,
                retrieval.fused_matches.iter(),
                &ranked_retrieval_branches,
                execution_budget.evidence_item_limit(),
            )
        } else {
            (retrieval.fused_scope.clone(), false)
        };
    let projection_owner_count = projection_scope.len();
    let owner_materialization_started = std::time::Instant::now();
    let resident = if !has_retrieval_clauses || structural_clauses.is_empty() {
        // Retrieval-only Search grounds exact regex bytes through the
        // generation-resident Topology anchor index. It must never bootstrap
        // providers or materialize owners on the request path.
        generation.resident_arc()
    } else if let Some(resident) = resident_semantic_scope(&generation, &projection_scope)? {
        resident
    } else {
        owner_materializer
            .ensure_candidates(
                request_id,
                workspace_identity,
                project_root,
                parser_artifact_root,
                generation.generation_digest(),
                &projection_scope,
                providers,
                runtime_search_service,
                workspace_registry,
            )
            .await?
            .into()
    };
    let owner_materialization_micros = owner_materialization_started.elapsed().as_micros();
    let structural_started = std::time::Instant::now();
    let mut structural_resource_receipt = None;
    let mut resource_permits = vec![retrieval_permit];
    if structural_clauses.is_empty() {
        let grounding_permit = generation
            .acquire_search_resources(
                agent_semantic_workspace_scheduler::RuntimeServerResourceRequest {
                    cpu: 1,
                    work_bytes: super::workspace_search_resources::scoped_source_work_bytes(
                        generation.as_ref(),
                        &retrieval.fused_scope,
                    ),
                    memory_bytes:
                        super::workspace_search_resources::structural_working_memory_bytes(
                            generation.as_ref(),
                            &retrieval.fused_scope,
                            retrieval.fused_matches.len(),
                        ),
                },
            )
            .await
            .map_err(AspClientOperationError::Message)?;
        structural_resource_receipt = Some(grounding_permit.receipt());
        let grounding_scope = projection_scope.clone();
        let grounding_matches = retrieval
            .fused_matches
            .iter()
            .filter(|matched| grounding_scope.contains(&matched.owner_path))
            .cloned()
            .collect::<Vec<_>>();
        let grounding_resident = Arc::clone(&resident);
        let grounding_task = generation
            .spawn_search_blocking("runtime-search-parser-grounding", move || {
                syntax_candidates_enclosing_rg_matches(
                    &grounding_resident,
                    &grounding_scope,
                    grounding_matches.iter(),
                )
                .map(|candidates| (candidates, grounding_permit))
            })
            .map_err(AspClientOperationError::Message)?;
        let (mut syntax_candidates, mut grounding_permit) =
            grounding_task.join().await.map_err(|error| {
                AspClientOperationError::Message(format!(
                    "resident parser grounding lane failed: {error}"
                ))
            })??;
        grounding_permit.release_cpu();
        resource_permits.push(grounding_permit);
        for candidate in &mut syntax_candidates {
            candidate.hit.tantivy = retrieval
                .tantivy_expressions_by_owner
                .get(&candidate.owner)
                .into_iter()
                .flatten()
                .cloned()
                .collect();
            if !candidate.hit.rg.is_empty() && !candidate.hit.tantivy.is_empty() {
                candidate.relation = "native-parser:rg-tantivy-fused".to_owned();
            } else if !candidate.hit.rg.is_empty() {
                candidate.relation = "native-parser:rg-grounded".to_owned();
            }
        }
        if let Some(clause) = retrieval.clauses.iter_mut().find(|clause| {
            matches!(
                clause.receipt.axis,
                WorkspaceSearchAxisKind::Rg
                    | WorkspaceSearchAxisKind::Tantivy
                    | WorkspaceSearchAxisKind::Topology
            )
        }) {
            clause.syntax_candidates.extend(syntax_candidates);
        }
    }
    let mut clause_executions = retrieval.clauses;
    let mut structural_scope = projection_scope;
    for (axis, block_index, priority_rank) in structural_clauses {
        let clause_started = std::time::Instant::now();
        let input_owner_count = structural_scope.len();
        let mut syntax_candidates = Vec::new();
        let receipt = match axis {
            SearchPlaybookClauseAxis::Rg
            | SearchPlaybookClauseAxis::Tantivy
            | SearchPlaybookClauseAxis::Topology => {
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
                let structural_resident = Arc::clone(&resident);
                let evidence = if let Some(evidence) = generation
                    .resident_syntax_scope_evidence(&block.plan.plan_digest, &structural_scope)
                    .map_err(AspClientOperationError::Message)?
                {
                    evidence
                } else {
                    let evidence = Arc::new(
                        execute_workspace_syntax_query_evidence(
                            &syntax_request,
                            generation.generation_digest(),
                            &structural_resident,
                            providers,
                            execution_budget.syntax_selector_limit(),
                            Some(&structural_scope),
                        )
                        .await?,
                    );
                    generation
                        .publish_resident_syntax_scope_evidence(
                            block.plan.plan_digest.clone(),
                            structural_scope.clone(),
                            Arc::clone(&evidence),
                        )
                        .map_err(AspClientOperationError::Message)?;
                    evidence
                };
                let owners = evidence
                    .iter()
                    .map(|item| item.owner.clone())
                    .collect::<BTreeSet<_>>()
                    .into_iter()
                    .collect();
                structural_scope = evidence.iter().map(|item| item.owner.clone()).collect();
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
                    SearchClauseMetrics {
                        input_owner_count,
                        marginal_owner_reduction: None,
                        elapsed_micros: elapsed_micros(clause_started),
                        coverage_complete: evidence.len()
                            < execution_budget.syntax_selector_limit(),
                        truncated: evidence.len() == execution_budget.syntax_selector_limit(),
                    },
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
                if !structural_scope.contains(&owner) {
                    return Err(AspClientOperationError::Message(format!(
                        "native-syntax selector owner is outside the preceding typed Search scope: {owner}"
                    )));
                }
                let (projections, _, diagnostics) = resident
                    .native_syntax_playbook_projection(std::slice::from_ref(&owner))
                    .map_err(AspClientOperationError::Message)?;
                if let Some(diagnostic) = diagnostics.first() {
                    return Err(AspClientOperationError::Message(format!(
                        "native-syntax selector owner is unavailable: reasonKind={} owner={}",
                        diagnostic.reason_kind, diagnostic.owner_path
                    )));
                }
                let admitted_selectors = projections
                    .iter()
                    .flat_map(|projection| projection.selectors.iter())
                    .map(|candidate| candidate.selector.as_str())
                    .collect::<BTreeSet<_>>();
                let admitted = admitted_selectors.contains(selector.as_str());
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
                structural_scope.clear();
                structural_scope.insert(owner.clone());
                complete_receipt(
                    WorkspaceSearchAxisKind::NativeSyntax,
                    block_index,
                    priority_rank,
                    vec![owner],
                    SearchClauseMetrics {
                        input_owner_count,
                        marginal_owner_reduction: None,
                        elapsed_micros: elapsed_micros(clause_started),
                        coverage_complete: true,
                        truncated: false,
                    },
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
    let graph_seed_scope = structural_scope;
    let topology_scope = if graph_query_clauses.is_empty() {
        graph_seed_scope.clone()
    } else {
        relation_neighbor_scope(
            &resident,
            &graph_seed_scope,
            execution_budget
                .graph_candidate_owner_limit()
                .max(graph_seed_scope.len()),
        )?
    };
    let structural_micros = structural_started.elapsed().as_micros();

    Ok(ProgressiveSearchEvidence {
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
        resource_permits,
    })
}

fn execute_default_retrieval_layout(
    plan: &WorkspaceSearchPlaybookPlan,
    generation: &RuntimeQueryGeneration,
    budget: &crate::runtime_search_execution_budget::RuntimeSearchExecutionBudget,
    clauses: &[(SearchPlaybookClauseAxis, usize, usize)],
) -> Result<RetrievalLayoutExecution, AspClientOperationError> {
    let retrieval_composition = retrieval_composition_kind(&plan.axes.composition);
    if clauses.is_empty() {
        if retrieval_composition != RetrievalCompositionKind::None {
            return Err(AspClientOperationError::Message(
                "Search composition declares retrieval without a normalized retrieval block"
                    .to_owned(),
            ));
        }
        return Ok(RetrievalLayoutExecution {
            clauses: Vec::new(),
            fused_scope: generation
                .resident()
                .indexed_owner_paths()
                .into_iter()
                .collect(),
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
    let topology_clauses = clauses
        .iter()
        .copied()
        .filter(|(axis, _, _)| *axis == SearchPlaybookClauseAxis::Topology)
        .collect::<Vec<_>>();
    if retrieval_composition == RetrievalCompositionKind::None {
        return Err(AspClientOperationError::Message(
            "Search retrieval blocks are absent from the normalized composition".to_owned(),
        ));
    }
    // Each backend is an independent set producer.  When both are present the
    // admitted Scheme intersection authorizes fusion; neither backend is
    // invented as a mandatory partner for the other.
    let mut tantivy_results = Vec::with_capacity(tantivy_clauses.len());
    for (_, block_index, priority_rank) in tantivy_clauses {
        let clause_started = std::time::Instant::now();
        let block = plan.axes.tantivy.get(block_index).ok_or_else(|| {
            AspClientOperationError::Message("Tantivy clause index is out of bounds".to_owned())
        })?;
        let result = execute_tantivy_block(
            block,
            &plan.routes,
            generation,
            None,
            budget.lexical_owner_limit(),
            budget.syntax_selector_limit(),
        )?;
        if retrieval_composition == RetrievalCompositionKind::Intersect {
            result.require_complete_fused_scope()?;
        }
        tantivy_results.push((
            block_index,
            priority_rank,
            block.join(" "),
            result,
            elapsed_micros(clause_started),
        ));
    }
    let mut topology_results = Vec::with_capacity(topology_clauses.len());
    for (_, block_index, priority_rank) in topology_clauses {
        let clause_started = std::time::Instant::now();
        let block = plan.axes.topology.get(block_index).ok_or_else(|| {
            AspClientOperationError::Message("Topology clause index is out of bounds".to_owned())
        })?;
        let result = execute_topology_block(
            block,
            &plan.routes,
            generation,
            budget.lexical_owner_limit() as usize,
        )?;
        if retrieval_composition == RetrievalCompositionKind::Intersect {
            require_complete_intersection_branch(
                WorkspaceSearchAxisKind::Topology,
                result.truncated,
            )?;
        }
        topology_results.push((
            block_index,
            priority_rank,
            result,
            elapsed_micros(clause_started),
        ));
    }
    // Execute rg against the complete indexed Workspace universe.  Tantivy is
    // not a legal rg prefilter merely because the Agent wrote an intersection:
    // that rewrite is sound only with a coverage witness R(p) ⊆ T(q, k).
    let rg_constraint_scope = generation
        .resident()
        .indexed_owner_paths()
        .into_iter()
        .collect();

    let mut rg_results = Vec::with_capacity(rg_clauses.len());
    for (_, block_index, priority_rank) in rg_clauses {
        let clause_started = std::time::Instant::now();
        let block = plan.axes.rg.get(block_index).ok_or_else(|| {
            AspClientOperationError::Message("rg clause index is out of bounds".to_owned())
        })?;
        let result = execute_runtime_resident_grep_blocks(
            generation.resident().resident_grep_corpus(),
            generation.resident().indexed_owner_count(),
            std::slice::from_ref(block),
            budget.rg_match_limit(),
            !generation
                .resident()
                .has_semantic_owner_materialization_authority(),
            |candidate_plan, limit| {
                resident_grep_candidate_scope(candidate_plan, &rg_constraint_scope, limit, || {
                    generation
                        .resident()
                        .resident_grep_candidate_owner_paths(candidate_plan, limit)
                })
            },
            |owner_path| generation.resident().resident_owner_bytes(owner_path),
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
        if retrieval_composition == RetrievalCompositionKind::Intersect {
            require_complete_intersection_branch(WorkspaceSearchAxisKind::Rg, result.truncated)?;
        }
        rg_results.push((
            block_index,
            priority_rank,
            result,
            elapsed_micros(clause_started),
        ));
    }

    let retrieval_branch_scopes = rg_results
        .iter()
        .map(|(block_index, _, result, _)| {
            (
                WorkspaceSearchAxisKind::Rg,
                *block_index,
                result.candidate_owner_paths.iter().cloned().collect(),
            )
        })
        .chain(
            tantivy_results
                .iter()
                .map(|(block_index, _, _, result, _)| {
                    (
                        WorkspaceSearchAxisKind::Tantivy,
                        *block_index,
                        result.owners.iter().cloned().collect(),
                    )
                }),
        )
        .chain(topology_results.iter().map(|(block_index, _, result, _)| {
            (
                WorkspaceSearchAxisKind::Topology,
                *block_index,
                result.owners.iter().cloned().collect(),
            )
        }))
        .collect::<Vec<(_, _, BTreeSet<String>)>>();
    let branch_marginal_reductions = branch_marginal_reductions(&retrieval_branch_scopes);
    let fused_scope = match retrieval_composition {
        RetrievalCompositionKind::Single => retrieval_branch_scopes
            .first()
            .map(|(_, _, scope)| scope.clone())
            .ok_or_else(|| {
                AspClientOperationError::Message(
                    "Search single retrieval composition has no branch".to_owned(),
                )
            })?,
        RetrievalCompositionKind::Intersect => intersect_clause_owner_scopes(
            &retrieval_branch_scopes
                .iter()
                .map(|(_, _, scope)| scope.clone())
                .collect::<Vec<_>>(),
        ),
        RetrievalCompositionKind::None => unreachable!("retrieval absence rejected above"),
    };
    let fused_matches = rg_results
        .iter()
        .flat_map(|(_, _, result, _)| result.grounding_matches.iter())
        .filter(|matched| fused_scope.contains(&matched.owner_path))
        .cloned()
        .collect::<Vec<_>>();
    let mut tantivy_expressions_by_owner = BTreeMap::<String, BTreeSet<String>>::new();
    for (_, _, expression, result, _) in &tantivy_results {
        for owner in &result.owners {
            tantivy_expressions_by_owner
                .entry(owner.clone())
                .or_default()
                .insert(expression.clone());
        }
    }
    let mut executions =
        Vec::with_capacity(rg_results.len() + tantivy_results.len() + topology_results.len());
    let input_owner_count = generation.resident().indexed_owner_paths().len();
    for (block_index, priority_rank, result, elapsed_micros) in rg_results {
        let candidate_owners = result.candidate_owner_paths;
        let marginal_owner_reduction = branch_marginal_reductions
            .get(&(WorkspaceSearchAxisKind::Rg, block_index))
            .copied();
        executions.push(SearchClauseExecution {
            priority_rank,
            receipt: complete_receipt(
                WorkspaceSearchAxisKind::Rg,
                block_index,
                priority_rank,
                candidate_owners,
                SearchClauseMetrics {
                    input_owner_count,
                    marginal_owner_reduction,
                    elapsed_micros,
                    coverage_complete: !result.truncated,
                    truncated: result.truncated,
                },
            ),
            syntax_candidates: Vec::new(),
        });
    }
    for (block_index, priority_rank, _, result, elapsed_micros) in tantivy_results {
        let marginal_owner_reduction = branch_marginal_reductions
            .get(&(WorkspaceSearchAxisKind::Tantivy, block_index))
            .copied();
        executions.push(SearchClauseExecution {
            priority_rank,
            receipt: complete_receipt(
                WorkspaceSearchAxisKind::Tantivy,
                block_index,
                priority_rank,
                result.owners,
                SearchClauseMetrics {
                    input_owner_count,
                    marginal_owner_reduction,
                    elapsed_micros,
                    coverage_complete: !result.truncated,
                    truncated: result.truncated,
                },
            ),
            syntax_candidates: result.syntax_candidates,
        });
    }
    for (block_index, priority_rank, mut result, elapsed_micros) in topology_results {
        result
            .syntax_candidates
            .retain(|candidate| fused_scope.contains(&candidate.owner));
        let marginal_owner_reduction = branch_marginal_reductions
            .get(&(WorkspaceSearchAxisKind::Topology, block_index))
            .copied();
        executions.push(SearchClauseExecution {
            priority_rank,
            receipt: complete_receipt(
                WorkspaceSearchAxisKind::Topology,
                block_index,
                priority_rank,
                result.owners,
                SearchClauseMetrics {
                    input_owner_count,
                    marginal_owner_reduction,
                    elapsed_micros,
                    coverage_complete: !result.truncated,
                    truncated: result.truncated,
                },
            ),
            syntax_candidates: result.syntax_candidates,
        });
    }
    Ok(RetrievalLayoutExecution {
        clauses: executions,
        fused_scope,
        fused_matches,
        tantivy_expressions_by_owner,
    })
}

#[cfg(test)]
#[path = "../tests/unit/runtime_workspace_search_playbook.rs"]
mod tests;
