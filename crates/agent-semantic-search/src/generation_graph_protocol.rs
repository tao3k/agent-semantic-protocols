//! Typed boundary for optional `asp-python-graphs` analysis of a Rust-owned
//! Search generation. These receipts are candidate evidence and never gate or
//! publish the base resident generation.

use std::collections::BTreeSet;

use agent_semantic_content_identity::{
    SourceSnapshotEvidence, provider_projection_relation::ProviderProjectedRelation,
    workspace_generation_evidence::WorkspaceGenerationEvidenceV1,
};
use serde::{Deserialize, Serialize};

use crate::{
    ContentSearchGenerationReceipt, SearchGenerationIdentity, canonical_blake3_digest,
    stable_graph_node_id,
};

pub const SEARCH_GENERATION_GRAPH_REQUEST_SCHEMA_ID: &str =
    "agent.semantic-protocols.search-generation-graph-request";
pub const SEARCH_GENERATION_GRAPH_RECEIPT_SCHEMA_ID: &str =
    "agent.semantic-protocols.search-generation-graph-receipt";

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SearchGenerationGraphRequest {
    pub schema_id: String,
    pub schema_version: String,
    pub identity: SearchGenerationIdentity,
    pub content_generation_digest: String,
    pub source_snapshot: SourceSnapshotEvidence,
    pub workspace_generation: WorkspaceGenerationEvidenceV1,
    pub owner_paths: Vec<String>,
    pub relations: Vec<ProviderProjectedRelation>,
}

impl SearchGenerationGraphRequest {
    pub fn new(
        content_generation: &ContentSearchGenerationReceipt,
        source_snapshot: SourceSnapshotEvidence,
        workspace_generation: WorkspaceGenerationEvidenceV1,
        owner_paths: impl IntoIterator<Item = String>,
        relations: impl IntoIterator<Item = ProviderProjectedRelation>,
    ) -> Result<Self, String> {
        let mut owner_paths = owner_paths.into_iter().collect::<Vec<_>>();
        owner_paths.sort();
        let mut relations = relations.into_iter().collect::<Vec<_>>();
        relations.sort();
        content_generation.validate()?;
        let request = Self {
            schema_id: SEARCH_GENERATION_GRAPH_REQUEST_SCHEMA_ID.to_owned(),
            schema_version: "1".to_owned(),
            identity: content_generation.identity().clone(),
            content_generation_digest: content_generation.content_generation_digest.clone(),
            source_snapshot,
            workspace_generation,
            owner_paths,
            relations,
        };
        request.validate()?;
        Ok(request)
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.schema_id != SEARCH_GENERATION_GRAPH_REQUEST_SCHEMA_ID || self.schema_version != "1"
        {
            return Err("search generation graph request schema mismatch".to_owned());
        }
        self.identity.validate()?;
        canonical_blake3_digest(&self.content_generation_digest)?;
        validate_source_snapshot(&self.source_snapshot)?;
        self.workspace_generation
            .validate_complete()
            .map_err(|error| error.to_string())?;
        if self.source_snapshot.root_digest != self.workspace_generation.root_digest
            || canonical_blake3_digest(&self.source_snapshot.root_digest)?
                != self.identity.source_root_digest
            || canonical_blake3_digest(&self.source_snapshot.provider_digest)?
                != self.identity.provider_digest
            || u64::try_from(self.source_snapshot.leaf_count).ok()
                != Some(self.workspace_generation.leaf_count)
        {
            return Err("search generation graph source identity drift".to_owned());
        }
        if self.owner_paths.is_empty()
            || self
                .owner_paths
                .windows(2)
                .any(|window| window[0] >= window[1])
            || self.owner_paths.iter().any(|owner| owner.trim().is_empty())
        {
            return Err(
                "search generation graph owners are not a non-empty canonical set".to_owned(),
            );
        }
        if u64::try_from(self.owner_paths.len()).ok() != Some(self.workspace_generation.owner_count)
        {
            return Err("search generation graph owner count drift".to_owned());
        }
        if self
            .relations
            .windows(2)
            .any(|window| window[0] >= window[1])
        {
            return Err("search generation graph relations are not a canonical set".to_owned());
        }
        let owners = self
            .owner_paths
            .iter()
            .map(String::as_str)
            .collect::<BTreeSet<_>>();
        for relation in &self.relations {
            relation.validate()?;
            for endpoint in [&relation.from, &relation.to] {
                if endpoint.kind == "owner" && !owners.contains(endpoint.id.as_str()) {
                    return Err(format!(
                        "search generation graph relation references an unadmitted owner: {}",
                        endpoint.id
                    ));
                }
            }
        }
        Ok(())
    }
}

fn validate_source_snapshot(snapshot: &SourceSnapshotEvidence) -> Result<(), String> {
    if snapshot.schema_id != "asp.source-snapshot.v1" || snapshot.algorithm != "blake3-merkle-v1" {
        return Err("search generation graph source snapshot schema mismatch".to_owned());
    }
    canonical_blake3_digest(&snapshot.root_digest)?;
    canonical_blake3_digest(&snapshot.provider_digest)?;
    for digest in [
        snapshot.base_root_digest.as_deref(),
        snapshot.dirty_paths_digest.as_deref(),
    ]
    .into_iter()
    .flatten()
    {
        canonical_blake3_digest(digest)?;
    }
    if snapshot.base_root_digest.is_some() != snapshot.dirty_paths_digest.is_some() {
        return Err("search generation graph overlay evidence is incomplete".to_owned());
    }
    Ok(())
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SearchGenerationGraphReceipt {
    pub schema_id: String,
    pub schema_version: String,
    pub identity: SearchGenerationIdentity,
    pub content_generation_digest: String,
    pub entry_owner_ids: Vec<String>,
    pub entry_node_ids: Vec<String>,
    pub candidate_owner_ids: Vec<String>,
    pub artifact_digest: String,
    pub complete: bool,
}

impl SearchGenerationGraphReceipt {
    pub fn validate_for(&self, request: &SearchGenerationGraphRequest) -> Result<(), String> {
        request.validate()?;
        if self.schema_id != SEARCH_GENERATION_GRAPH_RECEIPT_SCHEMA_ID
            || self.schema_version != "1"
            || !self.complete
        {
            return Err("search generation graph receipt schema or completion mismatch".to_owned());
        }
        if self.identity != request.identity {
            return Err("search generation graph receipt identity drift".to_owned());
        }
        if self.content_generation_digest != request.content_generation_digest {
            return Err("search generation graph receipt content generation drift".to_owned());
        }
        let expected_nodes = request
            .owner_paths
            .iter()
            .map(|owner| stable_graph_node_id("owner", owner))
            .collect::<Vec<_>>();
        if self.entry_owner_ids != request.owner_paths
            || self.candidate_owner_ids != request.owner_paths
            || self.entry_node_ids != expected_nodes
        {
            return Err("search generation graph receipt owner frontier drift".to_owned());
        }
        if canonical_blake3_digest(&self.artifact_digest).as_deref()
            != Ok(self.artifact_digest.as_str())
        {
            return Err("search generation graph receipt artifact digest is invalid".to_owned());
        }
        Ok(())
    }
}
