// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Runtime-owned graph ranking for one admitted resident search generation.

use agent_semantic_client_db::runtime_resident_read::RuntimeResidentReadClient;
use agent_semantic_search::ResidentGraphEvaluationBudget;
use agent_semantic_search::ResidentGraphEvaluationRequest;
use agent_semantic_search::evaluate_resident_graph_generation;
use agent_semantic_search::evaluate_resident_graph_relation_patterns;
use agent_semantic_search::stable_graph_node_id;
use serde_json::Value;
use serde_json::json;

pub(crate) struct WorkspaceSearchGraphExecution {
    pub(crate) candidate_owner_ids: Vec<String>,
}

/// Apply ordered Graph clauses to the candidate frontier produced by earlier
/// Search Playbook clauses. Rust evaluates only the exact published resident
/// generation and owns the candidate whitelist, limit, and public result.
#[allow(clippy::too_many_arguments)]
pub(crate) fn evaluate_resident_workspace_playbook_graph(
    request_id: &str,
    language_id: &str,
    query_clauses: &[agent_semantic_search::GraphNativeBlock],
    relation_patterns: &[agent_semantic_search::ResidentGraphRelationPattern],
    candidate_owners: &[String],
    budget: Option<ResidentGraphEvaluationBudget>,
    resident: &RuntimeResidentReadClient,
) -> Result<WorkspaceSearchGraphExecution, RuntimeSearchGraphFailure> {
    let graph_generation = resident
        .graph_generation()
        .map_err(RuntimeSearchGraphFailure::invalid)?
        .ok_or_else(RuntimeSearchGraphFailure::not_ready)?;
    let generation_request = graph_generation
        .generation_request()
        .map_err(RuntimeSearchGraphFailure::invalid)?;
    let generation_digest = generation_request
        .identity
        .generation_candidate_digest
        .clone();
    let mut seen_nodes = std::collections::BTreeSet::new();
    let entry_node_ids = candidate_owners
        .iter()
        .map(|owner| stable_graph_node_id("owner", owner))
        .filter(|node_id| seen_nodes.insert(node_id.clone()))
        .collect::<Vec<_>>();
    let started = tokio::time::Instant::now();
    let authority = resident.search_generation_authority();
    let evaluation = match (entry_node_ids.is_empty(), budget) {
        (false, Some(budget)) => Some(
            evaluate_resident_graph_relation_patterns(
                ResidentGraphEvaluationRequest {
                    operation_id: request_id,
                    generation_digest: &generation_digest,
                    source_snapshot: &authority.source_snapshot,
                    workspace_generation: &authority.workspace_generation,
                    entry_node_ids: &entry_node_ids,
                    generation_graph: &graph_generation,
                },
                candidate_owners,
                relation_patterns,
                budget,
            )
            .map_err(RuntimeSearchGraphFailure::invalid)?,
        ),
        // A generation with no Graph nodes or edges has an intentionally zero
        // cardinality-derived budget. It is a complete empty relation result,
        // not an invalid evaluator invocation.
        (_, None) | (true, Some(_)) => None,
    };
    graph_execution_receipt(
        language_id,
        query_clauses,
        generation_digest,
        evaluation,
        started,
    )
}

fn graph_execution_receipt(
    language_id: &str,
    query_clauses: &[agent_semantic_search::GraphNativeBlock],
    generation_digest: String,
    evaluation: Option<agent_semantic_search::ResidentGraphEvaluation>,
    started: tokio::time::Instant,
) -> Result<WorkspaceSearchGraphExecution, RuntimeSearchGraphFailure> {
    let mut seen_owners = std::collections::BTreeSet::new();
    let candidate_owner_ids = evaluation
        .as_ref()
        .into_iter()
        .flat_map(|evaluation| &evaluation.ranked_nodes)
        .filter_map(|node| node.owner_path.as_deref())
        .filter(|owner| seen_owners.insert((*owner).to_owned()))
        .map(ToOwned::to_owned)
        .collect::<Vec<_>>();
    let result_bytes = serde_json::to_vec(&json!({
        "generationDigest": generation_digest,
        "languageId": language_id,
        "queryClauses": query_clauses,
        "candidateOwnerIds": candidate_owner_ids,
    }))
    .map_err(|error| RuntimeSearchGraphFailure::invalid(error.to_string()))?;
    let _projection_digest = format!("blake3-256:{}", blake3::hash(&result_bytes).to_hex());
    let _elapsed_micros = u64::try_from(started.elapsed().as_micros()).unwrap_or(u64::MAX);
    Ok(WorkspaceSearchGraphExecution {
        candidate_owner_ids,
    })
}

/// A typed boundary error that the public ClientFrame route terminalizes.
pub(crate) struct RuntimeSearchGraphFailure {
    pub(crate) reason_kind: &'static str,
    pub(crate) message: String,
    pub(crate) details: Option<Value>,
}

/// Evaluate an intent-only graph request against the currently bound resident
/// generation. This path performs no Python/provider RPC, durable read, or
/// generation mutation.
#[allow(clippy::too_many_arguments)]
pub(crate) fn evaluate_resident_search_graph(
    request_id: &str,
    workspace_identity: &str,
    language_id: &str,
    provider_id: &str,
    generation_digest: &str,
    resident: &RuntimeResidentReadClient,
    params: &Value,
) -> Result<Value, RuntimeSearchGraphFailure> {
    let surface = params["surface"].as_str().expect("validated surface");
    let profile = params["profile"].as_str().expect("validated profile");
    let query_terms = params["queryTerms"]
        .as_array()
        .expect("validated queryTerms")
        .iter()
        .map(|term| term.as_str().expect("validated query term"))
        .collect::<Vec<_>>();
    let mut entry_node_ids = params["entryNodeIds"]
        .as_array()
        .expect("validated entryNodeIds")
        .iter()
        .map(|seed| seed.as_str().expect("validated seed").to_owned())
        .collect::<Vec<_>>();
    if !query_terms.is_empty() {
        let query = query_terms.join(" ");
        let authority = agent_semantic_search::ResidentSearchAuthority {
            language_id: language_id.into(),
            provider_id: provider_id.into(),
        };
        let lookup = resident
            .read_source_index(&query, Some(&authority), 100)
            .map_err(RuntimeSearchGraphFailure::invalid)?;
        entry_node_ids.extend(
            lookup
                .hits
                .iter()
                .map(|hit| stable_graph_node_id("owner", &hit.owner_path)),
        );
    }
    entry_node_ids.sort();
    entry_node_ids.dedup();

    let budget = &params["budget"];
    let max_depth = usize::try_from(budget["maxDepth"].as_u64().expect("validated maxDepth"))
        .map_err(|_| RuntimeSearchGraphFailure::invalid("maxDepth exceeds usize"))?;
    let max_nodes = usize::try_from(budget["maxNodes"].as_u64().expect("validated maxNodes"))
        .map_err(|_| RuntimeSearchGraphFailure::invalid("maxNodes exceeds usize"))?;
    let max_edges = usize::try_from(budget["maxEdges"].as_u64().expect("validated maxEdges"))
        .map_err(|_| RuntimeSearchGraphFailure::invalid("maxEdges exceeds usize"))?;
    let max_results = usize::try_from(budget["maxResults"].as_u64().expect("validated maxResults"))
        .map_err(|_| RuntimeSearchGraphFailure::invalid("maxResults exceeds usize"))?;
    let authority = resident.search_generation_authority();
    let evaluation = if entry_node_ids.is_empty() {
        None
    } else {
        Some(
            evaluate_resident_graph_generation(
                ResidentGraphEvaluationRequest {
                    operation_id: request_id,
                    generation_digest,
                    source_snapshot: &authority.source_snapshot,
                    workspace_generation: &authority.workspace_generation,
                    entry_node_ids: &entry_node_ids,
                    generation_graph: resident
                        .graph_generation()
                        .map_err(RuntimeSearchGraphFailure::invalid)?
                        .ok_or_else(RuntimeSearchGraphFailure::not_ready)?,
                },
                ResidentGraphEvaluationBudget {
                    max_depth,
                    max_nodes,
                    max_edges,
                    max_results,
                },
            )
            .map_err(RuntimeSearchGraphFailure::invalid)?,
        )
    };
    let ranked_nodes = evaluation
        .as_ref()
        .map(|evaluation| {
            evaluation
                .ranked_nodes
                .iter()
                .map(|node| {
                    json!({
                        "id": node.id,
                        "kind": node.kind,
                        "ownerPath": node.owner_path,
                        "score": node.score,
                        "distance": node.distance,
                    })
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let edges = evaluation
        .as_ref()
        .map(|evaluation| {
            evaluation
                .edges
                .iter()
                .map(|edge| {
                    json!({
                        "source": edge.source,
                        "target": edge.target,
                        "relation": edge.relation,
                    })
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let work = evaluation
        .as_ref()
        .map(|evaluation| evaluation.work)
        .unwrap_or_default();
    Ok(json!({
        "schemaId": "agent.semantic-protocols.semantic-graph-resident-evaluation-result",
        "schemaVersion": "1",
        "protocolId": "agent.semantic-protocols.search",
        "protocolVersion": "1",
        "packetKind": "resident-graph-evaluation-result",
        "state": "Ready",
        "surface": surface,
        "workspaceIdentity": workspace_identity,
        "generationDigest": generation_digest,
        "rootDigest": resident.source_root_digest(),
        "profile": profile,
        "entryNodeIds": entry_node_ids,
        "rankedNodes": ranked_nodes,
        "edges": edges,
        "workCounters": {
            "graphNodesVisited": work.visited_nodes,
            "graphEdgesVisited": work.visited_edges,
            "providerRpcCount": 0,
            "durableReadCount": 0,
            "generationMutationCount": 0,
        },
    }))
}

impl RuntimeSearchGraphFailure {
    fn invalid(message: impl Into<String>) -> Self {
        Self {
            reason_kind: "graph-search-invalid",
            message: message.into(),
            details: None,
        }
    }

    fn not_ready() -> Self {
        Self {
            reason_kind: "graph-not-ready",
            message: "exact-generation graph attachment is still building".to_owned(),
            details: Some(json!({"phase": "background-generation-graph"})),
        }
    }
}
