use std::path::{Path, PathBuf};

use crate::runtime_server::GraphTurboEvaluationBuilder;
use crate::runtime_server_workspace::RuntimeServerWorkspaceRegistry;
use crate::workspace_db_ipc::{RuntimeGraphFactSource, WorkspaceDbIpcResult};

pub(super) async fn evaluate(
    memory_registry: &RuntimeServerWorkspaceRegistry,
    builder: Option<&GraphTurboEvaluationBuilder>,
    workspace_identity: &str,
    project_root: String,
    message: serde_json::Value,
) -> WorkspaceDbIpcResult {
    let message = match memory_registry
        .projection_search_generation_authority(workspace_identity, Path::new(&project_root))
        .await
    {
        Ok(authority) => {
            canonical_rank_message(memory_registry, &authority, workspace_identity, &message).await
        }
        Err(message) => Err(message),
    };
    match (message, builder) {
        (Ok(message), Some(builder)) => match builder(
            workspace_identity.to_owned(),
            PathBuf::from(&project_root),
            message,
        )
        .await
        {
            Ok(receipt) => WorkspaceDbIpcResult::GraphTurboEvaluation {
                workspace_identity: workspace_identity.to_owned(),
                project_root,
                receipt,
            },
            Err(message) => WorkspaceDbIpcResult::Failed {
                code: "runtime-server-graph-turbo-evaluation-failed".to_owned(),
                message,
            },
        },
        (Ok(_), None) => WorkspaceDbIpcResult::Failed {
            code: "runtime-server-graph-turbo-evaluation-unavailable".to_owned(),
            message: "Runtime Server has no resident Graph Turbo evaluator".to_owned(),
        },
        (Err(message), _) => WorkspaceDbIpcResult::Failed {
            code: "runtime-server-graph-turbo-continuation-stale".to_owned(),
            message,
        },
    }
}

/// The Runtime Server is the only owner which can bind a graph request to a
/// live generation.  Search clients submit a rank intent; they never supply a
/// snapshot or generation continuation that could be stale or substituted.
async fn canonical_rank_message(
    memory_registry: &RuntimeServerWorkspaceRegistry,
    authority: &crate::runtime_server_workspace::WorkspaceSearchGenerationAuthority,
    workspace_identity: &str,
    message: &serde_json::Value,
) -> Result<serde_json::Value, String> {
    authority.validate_binding(workspace_identity, &authority.project_root)?;
    let source_root = authority.source_snapshot.root_digest.clone();
    let source_root = digest_with_algorithm(&source_root);
    let generation_digest = digest_with_algorithm(&authority.generation_digest);
    let request_bytes = serde_json::to_vec(message)
        .map_err(|error| format!("encode Graph Turbo rank intent: {error}"))?;
    let request_hash = blake3::hash(&request_bytes);
    let mut request_id_bytes = [0_u8; 8];
    request_id_bytes.copy_from_slice(&request_hash.as_bytes()[..8]);
    let request_id = u64::from_le_bytes(request_id_bytes).max(1);
    let query_terms = message
        .get("queryTerms")
        .cloned()
        .unwrap_or_else(|| serde_json::json!([]));
    let graph_sources = graph_sources(message)?;
    let facts = memory_registry
        .read_projection_graph_facts(
            workspace_identity,
            Path::new(&authority.project_root),
            &graph_sources,
        )
        .await?;
    if digest_with_algorithm(&facts.generation_digest) != generation_digest {
        return Err(
            "runtime graph facts generation does not match the admitted authority".to_owned(),
        );
    }
    let graph = graph_from_relations(&facts.relations);
    let seed_ids = graph_sources
        .iter()
        .map(|source| graph_node_id(&source.kind, &source.id))
        .collect::<Vec<_>>();
    let rank_payload = serde_json::json!({
        "graph": graph,
        "seedIds": seed_ids,
        "kindBudgets": message.get("kindBudgets").cloned().unwrap_or_else(|| serde_json::json!({})),
        "windowMerge": message.get("windowMerge").cloned().unwrap_or_else(|| serde_json::json!({})),
        "pathBudget": message.get("pathBudget").cloned().unwrap_or(serde_json::Value::Null),
        "pathMaxHops": message.get("pathMaxHops").cloned().unwrap_or(serde_json::Value::Null),
        "cache": message.get("cache").cloned().unwrap_or_else(|| serde_json::json!({})),
        "queryClauses": message.get("queryClauses").cloned().unwrap_or_else(|| serde_json::json!([])),
    });
    Ok(serde_json::json!({
        "schemaId": "agent.semantic-protocols.graph-turbo-resident-server",
        "schemaVersion": "1",
        "messageKind": "rank",
        "requestId": request_id,
        "workspaceIdentity": workspace_identity,
        "generationDigest": generation_digest,
        "pageRoots": {"source": source_root},
        "terms": query_terms,
        "profile": message.get("profile").cloned().unwrap_or_else(|| serde_json::json!("owner-query")),
        "budget": message.get("budget").cloned().unwrap_or_else(|| serde_json::json!(8)),
        "rankPayload": rank_payload,
    }))
}

/// The caller may identify small, typed graph roots, but no caller-supplied
/// graph content is admissible.  The resident generation segment remains the
/// sole authority for nodes and edges.
fn graph_sources(message: &serde_json::Value) -> Result<Vec<RuntimeGraphFactSource>, String> {
    let entries = message
        .get("graphSources")
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| "Graph Turbo rank intent requires typed graphSources".to_owned())?;
    let mut sources = entries
        .iter()
        .map(|entry| serde_json::from_value::<RuntimeGraphFactSource>(entry.clone()))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("decode Graph Turbo graph source: {error}"))?;
    sources.retain(|source| !source.kind.trim().is_empty() && !source.id.trim().is_empty());
    sources.sort_by(|left, right| (&left.kind, &left.id).cmp(&(&right.kind, &right.id)));
    sources.dedup();
    if sources.is_empty() {
        return Err("Graph Turbo rank intent has no typed graph sources".to_owned());
    }
    Ok(sources)
}

fn graph_from_relations(
    relations: &[agent_semantic_content_identity::provider_projection_relation::ProviderProjectedRelation],
) -> serde_json::Value {
    let mut nodes = std::collections::BTreeMap::new();
    let mut edges = std::collections::BTreeSet::new();
    for relation in relations {
        let source = graph_node_id(&relation.from.kind, &relation.from.id);
        let target = graph_node_id(&relation.to.kind, &relation.to.id);
        nodes.insert(
            source.clone(),
            serde_json::json!({
                "id": source,
                "kind": relation.from.kind,
                "role": "relation-endpoint",
                "value": relation.from.id,
                "action": relation.from.kind,
            }),
        );
        nodes.insert(
            target.clone(),
            serde_json::json!({
                "id": target,
                "kind": relation.to.kind,
                "role": "relation-endpoint",
                "value": relation.to.id,
                "action": relation.to.kind,
            }),
        );
        edges.insert((source, target, relation.kind.clone()));
    }
    serde_json::json!({
        "nodes": nodes.into_values().collect::<Vec<_>>(),
        "edges": edges.into_iter().map(|(source, target, relation)| serde_json::json!({
            "source": source,
            "target": target,
            "relation": relation,
        })).collect::<Vec<_>>(),
    })
}

fn graph_node_id(kind: &str, value: &str) -> String {
    let mut identifier = String::with_capacity(kind.len() + value.len() + 1);
    identifier.push_str(kind);
    identifier.push(':');
    for character in value.chars() {
        if character == '_' || character == '-' || character == '/' || character == '.' {
            identifier.push(character);
        } else if character.is_ascii_alphanumeric() {
            identifier.push(character.to_ascii_lowercase());
        } else {
            identifier.push('-');
        }
    }
    while identifier.ends_with('-') {
        identifier.pop();
    }
    if identifier.len() == kind.len() + 1 {
        identifier.push_str("node");
    }
    identifier
}

fn digest_with_algorithm(digest: &str) -> String {
    if digest.starts_with("blake3-256:") {
        digest.to_owned()
    } else {
        format!("blake3-256:{digest}")
    }
}
