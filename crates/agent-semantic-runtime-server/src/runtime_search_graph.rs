//! Runtime-owned graph ranking for one admitted resident search generation.

use agent_semantic_client_db::runtime_resident_read::RuntimeResidentReadClient;
use agent_semantic_search::{
    ResidentGraphEvaluationBudget, ResidentGraphEvaluationRequest, ResidentGraphSearchBudget,
    ResidentGraphSearchRequest, ResidentGraphSearchStage, ResidentSearchIntent,
    evaluate_resident_graph_generation, stable_graph_node_id,
};
use agent_semantic_search_projection::ResidentSearchHit;
use serde_json::{Value, json};

/// Lazily retain and evaluate the optional Python graph for relationship
/// intent under the exact immutable generation request already held by Rust.
/// No source/provider/durable read is performed here.
pub(crate) async fn evaluate_python_relationship_graph(
    request_id: &str,
    language_id: &str,
    query: &str,
    resident: &RuntimeResidentReadClient,
    lexical_hits: &[ResidentSearchHit],
    runtime_search_service: &agent_semantic_client_db::runtime_search_service::RuntimeSearchServiceHandle,
) -> Result<agent_semantic_search::SearchPlaybookPythonGraphExecution, RuntimeSearchGraphFailure> {
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
    let workspace_identity = generation_request.identity.workspace_id.clone();
    let generation_payload = serde_json::to_value(generation_request)
        .map_err(|error| RuntimeSearchGraphFailure::invalid(error.to_string()))?;
    let cancellation =
        agent_semantic_client_db::runtime_generation_cancellation::GenerationCancellation::new();
    let graph_receipt_value = runtime_search_service
        .generation_graph(
            format!("{request_id}-python-generation"),
            generation_payload,
            cancellation,
        )
        .await
        .map_err(RuntimeSearchGraphFailure::python)?;
    let graph_receipt: agent_semantic_search::SearchGenerationGraphReceipt =
        serde_json::from_value(graph_receipt_value)
            .map_err(|error| RuntimeSearchGraphFailure::python(error.to_string()))?;
    graph_receipt
        .validate_for(generation_request)
        .map_err(RuntimeSearchGraphFailure::python)?;

    let mut entry_node_ids = lexical_hits
        .iter()
        .map(|hit| stable_graph_node_id("owner", &hit.owner_path))
        .collect::<Vec<_>>();
    entry_node_ids.sort();
    entry_node_ids.dedup();
    let mut query_terms = query
        .split_whitespace()
        .filter(|term| !term.is_empty())
        .map(|term| term.chars().take(256).collect::<String>())
        .collect::<Vec<_>>();
    query_terms.sort();
    query_terms.dedup();
    query_terms.truncate(32);
    let evaluation_payload = json!({
        "schemaId": "agent.semantic-protocols.semantic-graph-resident-evaluation-request",
        "schemaVersion": "1",
        "protocolId": "agent.semantic-protocols.search",
        "protocolVersion": "1",
        "packetKind": "resident-graph-evaluation-request",
        "languageId": language_id,
        "surface": "search-playbook",
        "queryTerms": query_terms,
        "profile": "dependency",
        "entryNodeIds": entry_node_ids,
        "budget": {
            "maxDepth": 16,
            "maxNodes": 256,
            "maxEdges": 1024,
            "maxResults": 100,
        },
    });
    let started = tokio::time::Instant::now();
    let evaluation = runtime_search_service
        .evaluate_resident_graph(
            workspace_identity,
            generation_digest.clone(),
            format!("{request_id}-python-evaluate"),
            evaluation_payload,
            agent_semantic_client_db::runtime_generation_cancellation::GenerationCancellation::new(
            ),
        )
        .await
        .map_err(RuntimeSearchGraphFailure::python)?;
    if evaluation.get("projectId").and_then(Value::as_str)
        != Some(generation_request.identity.project_id.as_str())
        || evaluation.get("rootDigest").and_then(Value::as_str)
            != Some(generation_request.source_snapshot.root_digest.as_str())
        || evaluation
            .get("graphArtifactDigest")
            .and_then(Value::as_str)
            != Some(graph_receipt.artifact_digest.as_str())
    {
        return Err(RuntimeSearchGraphFailure::python(
            "asp-python-graphs resident evaluation identity drift",
        ));
    }
    let result = evaluation
        .get("result")
        .ok_or_else(|| RuntimeSearchGraphFailure::python("Python graph result is absent"))?;
    let mut candidate_owner_ids = result
        .get("rankedNodes")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|node| node.get("ownerPath").and_then(Value::as_str))
        .map(str::to_owned)
        .collect::<Vec<_>>();
    candidate_owner_ids.sort();
    candidate_owner_ids.dedup();
    let result_bytes = serde_json::to_vec(result)
        .map_err(|error| RuntimeSearchGraphFailure::python(error.to_string()))?;
    Ok(agent_semantic_search::SearchPlaybookPythonGraphExecution {
        generation_digest,
        projection_digest: format!("blake3-256:{}", blake3::hash(&result_bytes).to_hex()),
        candidate_owner_ids,
        elapsed_micros: u64::try_from(started.elapsed().as_micros()).unwrap_or(u64::MAX),
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
        "rootDigest": resident.owner_merkle_root_digest(),
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

    fn python(message: impl Into<String>) -> Self {
        Self {
            reason_kind: "python-graph-unavailable",
            message: message.into(),
            details: Some(json!({"phase": "exact-generation-python-graph"})),
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

fn unavailable_resident_graph_stage(
    generation_digest: &str,
    query: &str,
) -> Result<ResidentGraphSearchStage, RuntimeSearchGraphFailure> {
    let bytes = serde_json::to_vec(&json!({
        "generationDigest": generation_digest,
        "query": query,
        "state": "background-graph-not-ready",
    }))
    .map_err(|error| RuntimeSearchGraphFailure::invalid(error.to_string()))?;
    Ok(ResidentGraphSearchStage {
        generation_digest: generation_digest.to_owned(),
        result_digest: format!("blake3-256:{}", blake3::hash(&bytes).to_hex()),
        ranked_owner_paths: Vec::new(),
        elapsed_micros: 0,
        work: agent_semantic_search::ResidentGraphSearchWork::default(),
    })
}

/// Rank the resident lexical frontier directly over the immutable generation
/// graph. Warm queries execute bounded Rust work and never enter a provider or
/// secondary Runtime service session.
#[allow(clippy::too_many_arguments)]
pub(crate) fn rank_resident_search_frontier(
    request_id: &str,
    intent: ResidentSearchIntent,
    query: &str,
    language_id: &str,
    provider_id: &str,
    generation_digest: &str,
    resident: &RuntimeResidentReadClient,
    lexical_hits: &[ResidentSearchHit],
) -> Result<ResidentGraphSearchStage, RuntimeSearchGraphFailure> {
    let graph_generation = match resident
        .graph_generation()
        .map_err(RuntimeSearchGraphFailure::invalid)?
    {
        Some(generation) => generation,
        None if intent == ResidentSearchIntent::Relationship => {
            return Err(RuntimeSearchGraphFailure::not_ready());
        }
        None => return unavailable_resident_graph_stage(generation_digest, query),
    };
    if lexical_hits.is_empty() {
        let graph_digest = graph_generation.digest();
        let bytes = serde_json::to_vec(&json!({
            "generationDigest": generation_digest,
            "graphDigest": graph_digest,
            "query": query,
            "state": "complete-empty-frontier",
        }))
        .map_err(|error| RuntimeSearchGraphFailure::invalid(error.to_string()))?;
        return Ok(ResidentGraphSearchStage {
            generation_digest: generation_digest.to_owned(),
            result_digest: format!("blake3-256:{}", blake3::hash(&bytes).to_hex()),
            ranked_owner_paths: Vec::new(),
            elapsed_micros: 0,
            work: agent_semantic_search::ResidentGraphSearchWork::default(),
        });
    }

    let authority = resident.search_generation_authority();
    let source_snapshot = &authority.source_snapshot;
    let workspace_generation = &authority.workspace_generation;
    let request = ResidentGraphSearchRequest {
        operation_id: request_id,
        operation: intent.as_str(),
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
    .map_err(RuntimeSearchGraphFailure::invalid)
}
