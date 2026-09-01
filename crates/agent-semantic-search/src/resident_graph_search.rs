//! Generation-bound graph request projection for the resident lexical frontier.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use agent_semantic_content_identity::{
    ArtifactJson, SourceSnapshotEvidence, hash_normalized_json,
    provider_projection_relation::{ProviderProjectedRelation, ProviderProjectedRelationEndpoint},
    workspace_generation_evidence::WorkspaceGenerationEvidenceV1,
};
use agent_semantic_search_projection::{GraphTurboResultPacketV1, ResidentSearchHit};
use serde_json::{Value, json};

use crate::{source_index_lookup_terms, stable_graph_node_id};

/// Inputs admitted by one immutable Runtime search generation.
pub struct ResidentGraphSearchRequest<'a> {
    pub operation_id: &'a str,
    pub operation: &'a str,
    pub query: &'a str,
    pub language_id: &'a str,
    pub provider_id: &'a str,
    pub generation_digest: &'a str,
    pub source_snapshot: &'a SourceSnapshotEvidence,
    pub workspace_generation: &'a WorkspaceGenerationEvidenceV1,
    pub lexical_hits: &'a [ResidentSearchHit],
    /// Immutable whole-generation graph admitted with the resident mmap.
    /// Query-specific lexical hits are represented only by `seedIds`.
    pub generation_graph: &'a ResidentGraphGeneration,
}

/// Compact graph stage evidence retained by the v1 Runtime search receipt.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResidentGraphSearchStage {
    pub generation_digest: String,
    pub result_digest: String,
    pub ranked_owner_paths: Vec<String>,
    pub elapsed_micros: u64,
}

/// Canonical whole-generation graph shared by all warm queries.
#[derive(Clone, Debug)]
pub struct ResidentGraphGeneration {
    open_payload: Arc<Value>,
    digest: String,
    source_root_digest: String,
    leaf_count: u64,
    owner_count: u64,
    owner_node_ids: Arc<BTreeSet<String>>,
}

impl ResidentGraphGeneration {
    #[must_use]
    pub fn graph(&self) -> &Value {
        &self.open_payload["graph"]
    }

    #[must_use]
    pub fn shared_open_payload(&self) -> Arc<Value> {
        Arc::clone(&self.open_payload)
    }

    #[must_use]
    pub fn digest(&self) -> &str {
        &self.digest
    }

    fn contains_owner_path(&self, owner_path: &str) -> bool {
        self.owner_node_ids
            .contains(&stable_graph_node_id("owner", owner_path))
    }

    fn matches_generation_binding(
        &self,
        source_snapshot: &SourceSnapshotEvidence,
        workspace_generation: &WorkspaceGenerationEvidenceV1,
    ) -> bool {
        self.source_root_digest == source_snapshot.root_digest
            && self.source_root_digest == workspace_generation.root_digest
            && self.leaf_count == workspace_generation.leaf_count
            && self.owner_count == workspace_generation.owner_count
    }
}

/// Build the immutable graph payload loaded once per Runtime generation.
///
/// Owner nodes are admitted even when they have no relations, so any lexical
/// frontier can be expressed as a seed set without changing the generation
/// graph. Relations remain parser/provider-owned facts.
pub fn build_resident_graph_generation(
    source_snapshot: &SourceSnapshotEvidence,
    workspace_generation: &WorkspaceGenerationEvidenceV1,
    owner_paths: impl IntoIterator<Item = String>,
    relations: impl IntoIterator<Item = ProviderProjectedRelation>,
) -> Result<ResidentGraphGeneration, String> {
    if source_snapshot.root_digest != workspace_generation.root_digest
        || u64::try_from(source_snapshot.leaf_count).ok() != Some(workspace_generation.leaf_count)
    {
        return Err("resident generation graph source binding mismatch".to_owned());
    }
    let mut nodes = BTreeMap::<String, Value>::new();
    let mut edges = BTreeSet::<(String, String, String)>::new();
    let owner_paths = owner_paths.into_iter().collect::<BTreeSet<_>>();
    if u64::try_from(owner_paths.len()).ok() != Some(workspace_generation.owner_count) {
        return Err("resident generation graph owner count mismatch".to_owned());
    }
    let mut owner_node_ids = BTreeSet::new();
    for owner_path in &owner_paths {
        owner_node_ids.insert(stable_graph_node_id("owner", owner_path));
        insert_owner_node(&mut nodes, owner_path);
    }
    for relation in relations {
        relation.validate()?;
        for endpoint in [&relation.from, &relation.to] {
            if endpoint.kind == "owner" && !owner_paths.contains(&endpoint.id) {
                return Err(format!(
                    "resident generation graph relation references an unadmitted owner: {}",
                    endpoint.id
                ));
            }
        }
        let from_id = insert_relation_node(&mut nodes, &relation.from);
        let to_id = insert_relation_node(&mut nodes, &relation.to);
        edges.insert((from_id, to_id, relation.kind));
    }
    let graph = json!({
        "nodes": nodes.into_values().collect::<Vec<_>>(),
        "edges": edges.into_iter().map(|(source, target, relation)| json!({
            "source": source,
            "target": target,
            "relation": relation,
        })).collect::<Vec<_>>(),
    });
    let open_payload = json!({
        "graph": graph,
        "sourceSnapshot": source_snapshot,
        "workspaceGeneration": workspace_generation,
    });
    let artifact = ArtifactJson::from_serializable(&open_payload)
        .map_err(|error| format!("canonicalize resident generation graph: {error}"))?;
    let digest = format!("blake3-256:{}", hash_normalized_json(&artifact).value);
    Ok(ResidentGraphGeneration {
        open_payload: Arc::new(open_payload),
        digest,
        source_root_digest: source_snapshot.root_digest.clone(),
        leaf_count: workspace_generation.leaf_count,
        owner_count: workspace_generation.owner_count,
        owner_node_ids: Arc::new(owner_node_ids),
    })
}

/// Build the bounded Graph-Turbo request for one resident lexical frontier.
///
/// This function never reads source, opens a cache, or launches a process. The
/// caller supplies relations read from the same immutable generation as the
/// lexical hits.
pub fn build_resident_graph_search_request(
    request: ResidentGraphSearchRequest<'_>,
) -> Result<Option<Value>, String> {
    validate_generation_binding(&request)?;
    if request.lexical_hits.is_empty() {
        return Ok(None);
    }
    let query_terms = source_index_lookup_terms(request.query);
    if query_terms.is_empty() {
        return Err("resident graph search requires a non-empty lexical query".to_owned());
    }

    request
        .generation_graph
        .graph()
        .get("edges")
        .and_then(Value::as_array)
        .ok_or_else(|| "resident generation graph omitted edges".to_owned())?;
    let mut seed_ids = Vec::with_capacity(request.lexical_hits.len());
    for hit in request.lexical_hits {
        let owner_id = stable_graph_node_id("owner", &hit.owner_path);
        if !request
            .generation_graph
            .contains_owner_path(&hit.owner_path)
        {
            return Err(format!(
                "resident lexical owner is absent from generation graph: {}",
                hit.owner_path
            ));
        }
        seed_ids.push(owner_id);
    }
    let surface = match request.operation {
        "pipe" => "search-pipe",
        "rg" => "search-rg",
        _ => "search-lexical",
    };
    Ok(Some(json!({
        "schemaId": "agent.semantic-protocols.semantic-graph-turbo-request",
        "schemaVersion": "1",
        "protocolId": "agent.semantic-protocols.semantic-language",
        "protocolVersion": "1",
        "packetKind": "graph-turbo-request",
        "surface": surface,
        "sourceSnapshot": request.source_snapshot,
        "workspaceGeneration": request.workspace_generation,
        "queryTerms": query_terms,
        "profile": "owner-query",
        "algorithm": "typed-ppr-diverse",
        "seedIds": seed_ids,
        "budget": request.lexical_hits.len().clamp(1, 32),
        "cache": { "enabled": true },
        "requestId": request.operation_id,
        "producer": {
            "languageId": request.language_id,
            "providerId": request.provider_id,
        },
        "fields": {
            "generationDigest": request.generation_digest,
        },
    })))
}

/// Validate and reduce a Graph-Turbo result to Runtime search ordering evidence.
pub fn project_resident_graph_search_result(
    generation_digest: &str,
    result: GraphTurboResultPacketV1,
    elapsed_micros: u64,
) -> Result<ResidentGraphSearchStage, String> {
    let result_digest =
        agent_semantic_search_projection::SearchProjectionSource::semantic_digest(&result)
            .to_owned();
    let value = result.into_value();
    let ranked_nodes = value
        .get("rankedNodes")
        .and_then(Value::as_array)
        .ok_or_else(|| "graph search result omitted rankedNodes".to_owned())?;
    let mut seen = BTreeSet::new();
    let ranked_owner_paths = ranked_nodes
        .iter()
        .filter(|node| node.get("kind").and_then(Value::as_str) == Some("owner"))
        .filter_map(|node| {
            node.get("ownerPath")
                .or_else(|| node.get("path"))
                .or_else(|| node.get("value"))
                .and_then(Value::as_str)
        })
        .filter(|path| seen.insert((*path).to_owned()))
        .map(str::to_owned)
        .collect();
    Ok(ResidentGraphSearchStage {
        generation_digest: generation_digest.to_owned(),
        result_digest,
        ranked_owner_paths,
        elapsed_micros,
    })
}

fn validate_generation_binding(request: &ResidentGraphSearchRequest<'_>) -> Result<(), String> {
    if request.operation_id.is_empty()
        || request.language_id.is_empty()
        || request.provider_id.is_empty()
        || !request.generation_digest.starts_with("blake3-256:")
        || request.source_snapshot.root_digest != request.workspace_generation.root_digest
        || u64::try_from(request.source_snapshot.leaf_count).ok()
            != Some(request.workspace_generation.leaf_count)
        || !request
            .generation_graph
            .matches_generation_binding(request.source_snapshot, request.workspace_generation)
    {
        return Err("resident graph search generation binding mismatch".to_owned());
    }
    if request
        .lexical_hits
        .iter()
        .any(|hit| hit.owner_path.is_empty() || hit.owner_content_digest.is_empty())
    {
        return Err("resident graph search contains an invalid lexical hit".to_owned());
    }
    Ok(())
}

fn insert_relation_node(
    nodes: &mut BTreeMap<String, Value>,
    endpoint: &ProviderProjectedRelationEndpoint,
) -> String {
    if endpoint.kind == "owner" {
        return insert_owner_node(nodes, &endpoint.id);
    }
    let node_id = stable_graph_node_id(&endpoint.kind, &endpoint.id);
    nodes.entry(node_id.clone()).or_insert_with(|| {
        let action = match endpoint.kind.as_str() {
            "owner" => "owner",
            "item" => "code",
            "test" => "tests",
            _ => "topology",
        };
        json!({
            "id": node_id,
            "kind": endpoint.kind,
            "role": endpoint.kind,
            "value": endpoint.id,
            "action": action,
            "confidence": "parser",
        })
    });
    node_id
}

fn insert_owner_node(nodes: &mut BTreeMap<String, Value>, owner_path: &str) -> String {
    let node_id = stable_graph_node_id("owner", owner_path);
    nodes.entry(node_id.clone()).or_insert_with(|| {
        json!({
            "id": node_id,
            "kind": "owner",
            "role": "path",
            "value": owner_path,
            "action": "owner",
            "path": owner_path,
            "ownerPath": owner_path,
            "confidence": "exact",
        })
    });
    node_id
}
