// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Runtime execution of the Agent-authored progressive Search Playbook.

use std::collections::{BTreeMap, BTreeSet};
use std::time::Duration;

use agent_semantic_search::{
    SearchPlaybookClauseAxis, WorkspaceSearchAxisKind, WorkspaceSearchClauseReceipt,
    WorkspaceSearchPlaybookPlan, WorkspaceSearchSyntaxCandidate,
};
use gql_ir::GraphPatternElement;

use crate::RuntimeQueryGeneration;
use crate::runtime_asp_client::AspClientOperationError;
use crate::runtime_asp_client::syntax_query_route::execute_workspace_syntax_query_evidence;
use crate::runtime_cold_rg::{RuntimeRgMatch, execute_runtime_native_rg_blocks};

pub(super) struct ProgressiveSearchEvidence {
    pub(super) clause_receipts: Vec<WorkspaceSearchClauseReceipt>,
    pub(super) syntax_candidates: Vec<WorkspaceSearchSyntaxCandidate>,
    pub(super) graph_query_clauses: Vec<String>,
    pub(super) search_execution_elapsed_micros: u64,
}

pub(super) struct ProgressiveSearchProjection {
    pub(super) result: serde_json::Value,
    pub(super) elapsed_micros: u64,
}

pub(super) async fn execute_progressive_search_clauses(
    plan: &WorkspaceSearchPlaybookPlan,
    project_root: &std::path::Path,
    generation: &RuntimeQueryGeneration,
    providers: &[agent_semantic_search::WorkspaceSearchProvider],
    runtime_search_service: &agent_semantic_client_db::runtime_search_service::RuntimeSearchServiceHandle,
) -> Result<ProgressiveSearchEvidence, AspClientOperationError> {
    const INTERNAL_LIMIT: usize = 4096;
    let started = tokio::time::Instant::now();
    generation
        .require_search_playbook_topology_attachment()
        .map_err(|message| {
            AspClientOperationError::Terminal(
                agent_semantic_client_server::AspClientDispatchError {
                    reason_kind: "runtime-project-topology-attachment-missing".to_owned(),
                    message,
                    details: Some(serde_json::json!({
                        "failureStage": "runtime-project-topology-admission",
                        "runtimeGenerationDigest": generation.generation_digest(),
                        "terminalCount": 1
                    })),
                },
            )
        })?;

    let mut clause_receipts = Vec::new();
    let mut syntax_candidates = Vec::new();
    let mut graph_query_clauses = Vec::new();
    for clause in &plan.axes.clause_order {
        if clause.axis == SearchPlaybookClauseAxis::Graph {
            let block = plan.axes.graph.get(clause.block_index).ok_or_else(|| {
                AspClientOperationError::Message("Graph clause index is out of bounds".to_owned())
            })?;
            let source = block.argv.join(" ");
            compile_graph_relation_pattern(&block.language, &source)?;
            graph_query_clauses.push(source);
            continue;
        }

        let priority_rank = clause_receipts.len();
        let receipt = match clause.axis {
            SearchPlaybookClauseAxis::Fd => {
                let block = plan.axes.fd.get(clause.block_index).ok_or_else(|| {
                    AspClientOperationError::Message("fd clause index is out of bounds".to_owned())
                })?;
                let result = agent_semantic_search::execute_native_fd_blocks(
                    std::slice::from_ref(block),
                    &generation.resident().indexed_owner_paths(),
                )
                .map_err(AspClientOperationError::Message)?;
                complete_receipt(
                    WorkspaceSearchAxisKind::Fd,
                    clause.block_index,
                    priority_rank,
                    result.candidate_owner_paths,
                    true,
                    false,
                )
            }
            SearchPlaybookClauseAxis::Rg => {
                let block = plan.axes.rg.get(clause.block_index).ok_or_else(|| {
                    AspClientOperationError::Message("rg clause index is out of bounds".to_owned())
                })?;
                let result = execute_runtime_native_rg_blocks(
                    generation.resident().cold_rg_corpus(),
                    std::slice::from_ref(block),
                    INTERNAL_LIMIT as u32,
                    Duration::from_secs(2),
                )
                .await
                .map_err(AspClientOperationError::Message)?;
                syntax_candidates.extend(syntax_candidates_enclosing_rg_matches(
                    generation,
                    result.branch_matches.iter().flatten(),
                )?);
                complete_receipt(
                    WorkspaceSearchAxisKind::Rg,
                    clause.block_index,
                    priority_rank,
                    result.candidate_owner_paths,
                    !result.truncated,
                    result.truncated,
                )
            }
            SearchPlaybookClauseAxis::Tantivy => {
                let block = plan.axes.tantivy.get(clause.block_index).ok_or_else(|| {
                    AspClientOperationError::Message(
                        "Tantivy clause index is out of bounds".to_owned(),
                    )
                })?;
                let result =
                    execute_tantivy_block(block, &plan.routes, generation, INTERNAL_LIMIT as u32)?;
                syntax_candidates.extend(syntax_candidates_matching_tantivy_terms(
                    generation,
                    &result.matched_query_keys_by_owner,
                )?);
                complete_receipt(
                    WorkspaceSearchAxisKind::Tantivy,
                    clause.block_index,
                    priority_rank,
                    result.owners,
                    !result.truncated,
                    result.truncated,
                )
            }
            SearchPlaybookClauseAxis::Syntax => {
                let block = plan.axes.syntax.get(clause.block_index).ok_or_else(|| {
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
                        languages: plan.languages.clone(),
                        documents: plan.documents.clone(),
                        workspace: Some(project_root.display().to_string()),
                        syntax: vec![
                            agent_semantic_client_protocol::AspClientSearchPlaybookSyntaxBlock {
                                producer: block.producer.clone(),
                                argv: block.argv.clone(),
                            },
                        ],
                        projection: "selectors".to_owned(),
                    };
                let prior_candidate_owners = syntax_owner_scope(&clause_receipts);
                let evidence = execute_workspace_syntax_query_evidence(
                    &syntax_request,
                    project_root,
                    generation,
                    providers,
                    runtime_search_service,
                    INTERNAL_LIMIT,
                    prior_candidate_owners.as_ref(),
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
                    }
                }));
                complete_receipt(
                    WorkspaceSearchAxisKind::Syntax,
                    clause.block_index,
                    priority_rank,
                    owners,
                    evidence.len() < INTERNAL_LIMIT,
                    evidence.len() == INTERNAL_LIMIT,
                )
            }
            SearchPlaybookClauseAxis::NativeSyntax => {
                let selector =
                    plan.axes
                        .native_syntax
                        .get(clause.block_index)
                        .ok_or_else(|| {
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
                let (projections, _, diagnostics) = generation
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
                });
                complete_receipt(
                    WorkspaceSearchAxisKind::NativeSyntax,
                    clause.block_index,
                    priority_rank,
                    vec![owner],
                    true,
                    false,
                )
            }
            SearchPlaybookClauseAxis::Graph => unreachable!("Graph is a fan-in barrier"),
        };
        clause_receipts.push(receipt);
    }

    Ok(ProgressiveSearchEvidence {
        clause_receipts,
        syntax_candidates,
        graph_query_clauses,
        search_execution_elapsed_micros: elapsed_micros(started),
    })
}

pub(super) async fn synthesize_progressive_search_projection(
    request_id: &str,
    language_id: &str,
    evidence: ProgressiveSearchEvidence,
    generation: &RuntimeQueryGeneration,
    runtime_search_service: &agent_semantic_client_db::runtime_search_service::RuntimeSearchServiceHandle,
) -> Result<ProgressiveSearchProjection, AspClientOperationError> {
    const GRAPH_CANDIDATE_FRONTIER_LIMIT: usize = 4096;
    let started = tokio::time::Instant::now();
    let graph_fan_in = if evidence.graph_query_clauses.is_empty() {
        None
    } else {
        let mut seen = BTreeSet::new();
        let mut candidate_owners = Vec::new();
        let mut candidate_frontier_truncated = false;
        'clauses: for receipt in &evidence.clause_receipts {
            for owner in &receipt.candidate_owners {
                if seen.insert(owner.as_str()) {
                    if candidate_owners.len() == GRAPH_CANDIDATE_FRONTIER_LIMIT {
                        candidate_frontier_truncated = true;
                        break 'clauses;
                    }
                    candidate_owners.push(owner.clone());
                }
            }
        }
        let graph = crate::runtime_search_graph::evaluate_python_workspace_playbook_graph(
            request_id,
            language_id,
            &evidence.graph_query_clauses,
            &candidate_owners,
            30,
            generation.resident(),
            runtime_search_service,
        )
        .await
        .map_err(|error| {
            AspClientOperationError::Terminal(
                agent_semantic_client_server::AspClientDispatchError {
                    reason_kind: error.reason_kind.to_owned(),
                    message: error.message,
                    details: error.details,
                },
            )
        })?;
        let truncated = candidate_frontier_truncated || graph.candidate_owner_ids.len() == 30;
        Some(agent_semantic_search::WorkspaceSearchGraphFanIn {
            ranked_candidate_owners: graph.candidate_owner_ids,
            applied_clause_count: evidence.graph_query_clauses.len(),
            complete: true,
            truncated,
        })
    };
    let workspace_result = agent_semantic_search::synthesize_workspace_search_playbook_result(
        evidence.clause_receipts,
        evidence.syntax_candidates,
        graph_fan_in,
    )?;
    let workspace_result = serde_json::to_value(workspace_result)
        .map_err(|error| AspClientOperationError::Message(error.to_string()))?;
    let attachment = generation
        .require_search_playbook_topology_attachment()
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
        elapsed_micros: elapsed_micros(started),
    })
}

fn elapsed_micros(started: tokio::time::Instant) -> u64 {
    started.elapsed().as_micros().min(u128::from(u64::MAX)) as u64
}

fn syntax_owner_scope(prior_receipts: &[WorkspaceSearchClauseReceipt]) -> Option<BTreeSet<String>> {
    (!prior_receipts.is_empty()).then(|| {
        prior_receipts
            .iter()
            .flat_map(|receipt| receipt.candidate_owners.iter().cloned())
            .collect()
    })
}

fn syntax_candidates_enclosing_rg_matches<'a>(
    generation: &RuntimeQueryGeneration,
    matches: impl IntoIterator<Item = &'a RuntimeRgMatch>,
) -> Result<Vec<WorkspaceSearchSyntaxCandidate>, AspClientOperationError> {
    let matches = matches.into_iter().cloned().collect::<Vec<_>>();
    let owners = matches
        .iter()
        .map(|item| item.owner_path.clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    if owners.is_empty() {
        return Ok(Vec::new());
    }
    let (projections, _, _) = generation
        .native_syntax_playbook_projection(&owners)
        .map_err(AspClientOperationError::Message)?;
    let projections = projections
        .into_iter()
        .map(|projection| (projection.owner_path.clone(), projection))
        .collect::<BTreeMap<_, _>>();
    let mut line_ranges = BTreeMap::new();
    for owner in &owners {
        let snapshot = generation
            .resident()
            .owner_snapshot(owner)
            .map_err(AspClientOperationError::Message)?
            .ok_or_else(|| {
                AspClientOperationError::Message(format!(
                    "rg syntax mapping owner disappeared: {owner}"
                ))
            })?;
        line_ranges.insert(owner.clone(), source_line_ranges(&snapshot.bytes));
    }

    let mut candidates = Vec::new();
    let mut seen_selectors = BTreeSet::new();
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
        if let Some(selector) = enclosing
            && seen_selectors.insert(selector.selector.clone())
        {
            candidates.push(WorkspaceSearchSyntaxCandidate {
                owner: item.owner_path,
                selector: selector.selector.clone(),
                relation: "syntax-encloses:rg-match".to_owned(),
            });
        }
    }
    Ok(candidates)
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

fn syntax_candidates_matching_tantivy_terms(
    generation: &RuntimeQueryGeneration,
    matched_query_keys_by_owner: &BTreeMap<String, BTreeSet<String>>,
) -> Result<Vec<WorkspaceSearchSyntaxCandidate>, AspClientOperationError> {
    let owners = matched_query_keys_by_owner
        .keys()
        .cloned()
        .collect::<Vec<_>>();
    if owners.is_empty() {
        return Ok(Vec::new());
    }
    let (projections, _, _) = generation
        .native_syntax_playbook_projection(&owners)
        .map_err(AspClientOperationError::Message)?;
    let mut candidates = Vec::new();
    let mut seen_selectors = BTreeSet::new();
    for projection in projections {
        let Some(matched_keys) = matched_query_keys_by_owner.get(&projection.owner_path) else {
            continue;
        };
        for selector in projection.selectors {
            if selector_matches_tantivy_terms(&selector, matched_keys)
                && seen_selectors.insert(selector.selector.clone())
            {
                candidates.push(WorkspaceSearchSyntaxCandidate {
                    owner: projection.owner_path.clone(),
                    selector: selector.selector,
                    relation: "syntax-query-key-match:tantivy".to_owned(),
                });
            }
        }
    }
    Ok(candidates)
}

fn selector_matches_tantivy_terms(
    selector: &agent_semantic_search::NativeSyntaxSelector,
    matched_keys: &BTreeSet<String>,
) -> bool {
    selector
        .query_keys
        .iter()
        .map(|key| key.to_ascii_lowercase())
        .any(|key| matched_keys.contains(&key))
}

fn compile_graph_relation_pattern(
    language: &str,
    source: &str,
) -> Result<(), AspClientOperationError> {
    if !matches!(language, "gql" | "pgql") {
        return Err(AspClientOperationError::Message(format!(
            "graph language is not registered by the V1 relation adapter: {language}"
        )));
    }
    let catalog = gql_catalog::Catalog::new(
        gql_catalog::CatalogName("asp-workspace".to_owned()),
        Vec::new(),
        Vec::new(),
    );
    let compilation = gql_compiler::Compiler.compile("search-playbook.gql", source, &catalog);
    if !compilation.analysis.diagnostics.is_empty() {
        return Err(AspClientOperationError::Message(format!(
            "{language} graph query is invalid: {:?}",
            compilation.analysis.diagnostics
        )));
    }
    let query = compilation.analysis.ir.ok_or_else(|| {
        AspClientOperationError::Message(format!(
            "{language} graph query produced no executable IR"
        ))
    })?;
    if query.matches.len() != 1
        || !query.optional_matches.is_empty()
        || !query.filters.is_empty()
        || !query.mutations.is_empty()
        || !query.set_operations.is_empty()
    {
        return Err(AspClientOperationError::Message(
            "V1 Graph fan-in admits one unfiltered MATCH relation pattern".to_owned(),
        ));
    }
    let graph_match = &query.matches[0];
    if graph_match.paths.len() != 1 || graph_match.paths[0].elements.len() != 3 {
        return Err(AspClientOperationError::Message(
            "V1 Graph fan-in requires MATCH (left)-[:RELATION]->(right)".to_owned(),
        ));
    }
    let elements = &graph_match.paths[0].elements;
    let (
        GraphPatternElement::Node(left),
        GraphPatternElement::Edge(edge),
        GraphPatternElement::Node(right),
    ) = (&elements[0], &elements[1], &elements[2])
    else {
        return Err(AspClientOperationError::Message(
            "V1 Graph fan-in requires a node-edge-node path".to_owned(),
        ));
    };
    if edge.labels.len() != 1
        || edge.quantifier.is_some()
        || !edge.properties.is_empty()
        || edge.predicate.is_some()
        || !left.properties.is_empty()
        || left.predicate.is_some()
        || !right.properties.is_empty()
        || right.predicate.is_some()
        || left.labels.is_empty()
        || right.labels.is_empty()
    {
        return Err(AspClientOperationError::Message(
            "V1 Graph fan-in requires labelled endpoints, one edge label, and no inline predicates"
                .to_owned(),
        ));
    }
    Ok(())
}

struct TantivyClauseResult {
    owners: Vec<String>,
    matched_query_keys_by_owner: BTreeMap<String, BTreeSet<String>>,
    truncated: bool,
}

fn execute_tantivy_block(
    block: &[String],
    routes: &[agent_semantic_search::WorkspaceSearchPlaybookRoute],
    generation: &RuntimeQueryGeneration,
    limit: u32,
) -> Result<TantivyClauseResult, AspClientOperationError> {
    let mut owners = BTreeSet::new();
    let mut matched_query_keys_by_owner = BTreeMap::<String, BTreeSet<String>>::new();
    let mut truncated = false;
    let expression = block.join(" ");
    for branch in expression.split('|').map(str::trim) {
        if branch.is_empty() {
            return Err(AspClientOperationError::Message(
                "Tantivy reasoning branches cannot be empty".to_owned(),
            ));
        }
        for route in routes {
            let language =
                agent_semantic_client_core::LanguageId::try_from(route.language_id.as_str())
                    .map_err(|error| AspClientOperationError::Message(error.to_string()))?;
            let result = generation
                .resident()
                .read_source_index_for_language(branch, &language, limit)
                .map_err(AspClientOperationError::Message)?;
            truncated |= result.hits.len() == limit as usize;
            for hit in &result.hits {
                owners.insert(hit.owner_path.clone());
                matched_query_keys_by_owner
                    .entry(hit.owner_path.clone())
                    .or_default()
                    .extend(hit.query_keys.iter().map(|key| key.to_ascii_lowercase()));
            }
        }
    }
    Ok(TantivyClauseResult {
        owners: owners.into_iter().collect(),
        matched_query_keys_by_owner,
        truncated,
    })
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
