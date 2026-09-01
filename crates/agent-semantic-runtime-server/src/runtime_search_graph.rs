//! Runtime-owned graph ranking for one admitted resident search generation.

use agent_semantic_client_db::runtime_resident_read::RuntimeResidentReadClient;
use agent_semantic_search::{
    ResidentGraphEvaluationBudget, ResidentGraphEvaluationRequest, ResidentGraphSearchBudget,
    ResidentGraphSearchRequest, ResidentGraphSearchStage, evaluate_resident_graph_generation,
    stable_graph_node_id,
};
use agent_semantic_search_projection::ResidentSearchHit;
use serde_json::{Value, json};

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
    let mut seed_ids = params["seedIds"]
        .as_array()
        .expect("validated seedIds")
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
        seed_ids.extend(
            lookup
                .hits
                .iter()
                .map(|hit| stable_graph_node_id("owner", &hit.owner_path)),
        );
    }
    seed_ids.sort();
    seed_ids.dedup();

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
    let evaluation = if seed_ids.is_empty() {
        None
    } else {
        Some(
            evaluate_resident_graph_generation(
                ResidentGraphEvaluationRequest {
                    operation_id: request_id,
                    generation_digest,
                    source_snapshot: &authority.source_snapshot,
                    workspace_generation: &authority.workspace_generation,
                    seed_ids: &seed_ids,
                    generation_graph: resident.graph_generation(),
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
        "rootDigest": resident.root_digest(),
        "profile": profile,
        "seedIds": seed_ids,
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
}

/// Rank the resident lexical frontier directly over the immutable generation
/// graph. Warm queries execute bounded Rust work and never enter a provider or
/// secondary Runtime service session.
#[allow(clippy::too_many_arguments)]
pub(crate) fn rank_resident_search_frontier(
    request_id: &str,
    operation: &str,
    query: &str,
    language_id: &str,
    provider_id: &str,
    generation_digest: &str,
    resident: &RuntimeResidentReadClient,
    lexical_hits: &[ResidentSearchHit],
) -> Result<Option<ResidentGraphSearchStage>, RuntimeSearchGraphFailure> {
    if operation != "pipe" || lexical_hits.is_empty() {
        return Ok(None);
    }

    let authority = resident.search_generation_authority();
    let source_snapshot = &authority.source_snapshot;
    let workspace_generation = &authority.workspace_generation;
    let graph_generation = resident.graph_generation();
    let request = ResidentGraphSearchRequest {
        operation_id: request_id,
        operation,
        query,
        language_id,
        provider_id,
        generation_digest,
        source_snapshot,
        workspace_generation,
        lexical_hits,
        generation_graph: graph_generation,
    };
    agent_semantic_search::rank_resident_graph_generation(
        request,
        ResidentGraphSearchBudget {
            max_nodes: 256,
            max_edges: 1024,
            max_frontier: 128,
            max_results: agent_semantic_search::RUNTIME_SEARCH_SELECTOR_OWNER_LIMIT,
        },
    )
    .map(Some)
    .map_err(RuntimeSearchGraphFailure::invalid)
}
