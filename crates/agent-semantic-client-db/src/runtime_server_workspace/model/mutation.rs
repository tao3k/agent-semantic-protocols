// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Generation mutation and exact-selector overlay contracts.

use serde::{Deserialize, Serialize};

use super::{ExactProjectionKind, RuntimeProjectionScope, WorkspaceOwnerSnapshot, validate_owners};

pub const WORKSPACE_OWNER_CONTENT_MUTATION_SCHEMA_ID: &str =
    "agent.semantic-protocols.workspace-owner-content-mutation";
pub const WORKSPACE_OWNER_CONTENT_MUTATION_RECEIPT_SCHEMA_ID: &str =
    "agent.semantic-protocols.workspace-owner-content-mutation-receipt";
pub const WORKSPACE_OWNER_TOPOLOGY_REBIND_SCHEMA_ID: &str =
    "agent.semantic-protocols.workspace-owner-topology-rebind";
pub const WORKSPACE_OWNER_TOPOLOGY_REBIND_RECEIPT_SCHEMA_ID: &str =
    "agent.semantic-protocols.workspace-owner-topology-rebind-receipt";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WorkspaceOwnerContentUpsertV1 {
    pub previous_content_digest: Option<String>,
    pub owner: WorkspaceOwnerSnapshot,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WorkspaceOwnerContentRemovalV1 {
    pub owner_path: String,
    pub previous_content_digest: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WorkspaceOwnerContentMutationV1 {
    pub schema_id: String,
    pub schema_version: String,
    pub mutation_id: String,
    pub base_generation_digest: String,
    pub upserts: Vec<WorkspaceOwnerContentUpsertV1>,
    pub removals: Vec<WorkspaceOwnerContentRemovalV1>,
}

impl WorkspaceOwnerContentMutationV1 {
    pub fn validate(&self) -> Result<(), String> {
        if self.schema_id != WORKSPACE_OWNER_CONTENT_MUTATION_SCHEMA_ID
            || self.schema_version != "1"
        {
            return Err("workspace owner content mutation identity mismatch".to_owned());
        }
        if self.mutation_id.trim().is_empty()
            || !self.base_generation_digest.starts_with("blake3-256:")
            || self.upserts.is_empty() && self.removals.is_empty()
        {
            return Err("workspace owner content mutation scope is incomplete".to_owned());
        }
        let mut paths = std::collections::BTreeSet::new();
        for upsert in &self.upserts {
            validate_owners(std::slice::from_ref(&upsert.owner))?;
            if !upsert.owner.selectors.is_empty()
                || upsert.owner.native_syntax_diagnostic.is_some()
                || !paths.insert(upsert.owner.owner_path.as_str())
            {
                return Err(
                    "workspace owner content mutation upsert must be a unique byte-only owner"
                        .to_owned(),
                );
            }
        }
        for removal in &self.removals {
            if removal.owner_path.trim().is_empty()
                || !removal.previous_content_digest.starts_with("blake3-256:")
                || !paths.insert(removal.owner_path.as_str())
            {
                return Err(
                    "workspace owner content mutation removal must be unique and content-bound"
                        .to_owned(),
                );
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WorkspaceOwnerContentMutationReceiptV1 {
    pub schema_id: String,
    pub schema_version: String,
    pub mutation_id: String,
    pub workspace_identity: String,
    pub base_generation_digest: String,
    pub resident_generation_digest: String,
    pub owner_identity_root_digest: String,
    pub upserted_owner_count: usize,
    pub removed_owner_count: usize,
    pub touched_leaf_count: usize,
    pub written_node_count: usize,
    pub reused_node_count: usize,
    pub full_merkle_rebuilds: usize,
}

impl WorkspaceOwnerContentMutationReceiptV1 {
    pub fn validate(&self) -> Result<(), String> {
        if self.schema_id != WORKSPACE_OWNER_CONTENT_MUTATION_RECEIPT_SCHEMA_ID
            || self.schema_version != "1"
            || self.mutation_id.trim().is_empty()
            || self.workspace_identity.trim().is_empty()
            || !self.base_generation_digest.starts_with("blake3-256:")
            || !self.resident_generation_digest.starts_with("blake3-256:")
            || self.owner_identity_root_digest.len() != 64
            || self.upserted_owner_count + self.removed_owner_count == 0
            || self.touched_leaf_count != self.upserted_owner_count + self.removed_owner_count
            || self.full_merkle_rebuilds != 0
        {
            return Err("workspace owner content mutation receipt is invalid".to_owned());
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WorkspaceOwnerTopologyRebindV1 {
    pub schema_id: String,
    pub schema_version: String,
    pub rebind_id: String,
    pub base_generation_digest: String,
    pub owners: Vec<WorkspaceOwnerSnapshot>,
    pub relations: Vec<crate::ClientDbSourceIndexOwnedRelation>,
}

impl WorkspaceOwnerTopologyRebindV1 {
    pub fn validate(&self) -> Result<(), String> {
        if self.schema_id != WORKSPACE_OWNER_TOPOLOGY_REBIND_SCHEMA_ID
            || self.schema_version != "1"
            || self.rebind_id.trim().is_empty()
            || !self.base_generation_digest.starts_with("blake3-256:")
            || self.owners.is_empty()
        {
            return Err("workspace owner topology rebind identity is invalid".to_owned());
        }
        validate_owners(&self.owners)?;
        let mut owners = std::collections::BTreeSet::new();
        for owner in &self.owners {
            if !owners.insert(owner.owner_path.as_str()) {
                return Err("workspace owner topology rebind owners must be unique".to_owned());
            }
        }
        for relation in &self.relations {
            relation.relation.validate()?;
            if !owners.contains(relation.owner_path.as_str()) {
                return Err(
                    "workspace owner topology rebind relation is outside owner membership"
                        .to_owned(),
                );
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WorkspaceOwnerTopologyRebindReceiptV1 {
    pub schema_id: String,
    pub schema_version: String,
    pub rebind_id: String,
    pub workspace_identity: String,
    pub base_generation_digest: String,
    pub resident_generation_digest: String,
    pub rebound_owner_count: usize,
    pub topology_node_count: usize,
    pub relation_count: usize,
}

impl WorkspaceOwnerTopologyRebindReceiptV1 {
    pub fn validate(&self) -> Result<(), String> {
        if self.schema_id != WORKSPACE_OWNER_TOPOLOGY_REBIND_RECEIPT_SCHEMA_ID
            || self.schema_version != "1"
            || self.rebind_id.trim().is_empty()
            || self.workspace_identity.trim().is_empty()
            || !self.base_generation_digest.starts_with("blake3-256:")
            || !self.resident_generation_digest.starts_with("blake3-256:")
            || self.rebound_owner_count == 0
        {
            return Err("workspace owner topology rebind receipt is invalid".to_owned());
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceRuntimeSelectorOverlay {
    pub projection_kind: ExactProjectionKind,
    pub structural_selector: String,
    pub owner_path: String,
    pub owner_content_digest: String,
    pub byte_start: usize,
    pub byte_end: usize,
    pub projection_bytes: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceRuntimeSelectorRebind {
    pub owner: WorkspaceOwnerSnapshot,
    pub overlay: WorkspaceRuntimeSelectorOverlay,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceRuntimeSelectorOverlayReceipt {
    pub schema_id: String,
    pub schema_version: String,
    pub workspace_identity: String,
    pub generation_digest: String,
    pub projection_kind: ExactProjectionKind,
    pub structural_selector: String,
    pub inserted: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "state",
    rename_all = "kebab-case",
    rename_all_fields = "camelCase"
)]
pub enum WorkspaceRuntimeSelectorRead {
    GenerationMissing,
    ProviderProjection {
        owner_content_digest: String,
        resolved_selector: String,
        bytes: Vec<u8>,
    },
    Projection {
        generation_digest: String,
        root_digest: String,
        resolved_selector: String,
        bytes: Vec<u8>,
    },
    ProjectionMissing {
        generation_digest: String,
        root_digest: String,
        resolved_selector: String,
    },
    ProjectionScopeOmitted {
        generation_digest: String,
        root_digest: String,
        resolved_selector: String,
        projection_scope: RuntimeProjectionScope,
        owner_content_digest: String,
    },
    OwnerForRepair {
        generation_digest: String,
        root_digest: String,
        owner: WorkspaceOwnerSnapshot,
    },
    OwnerMissing {
        generation_digest: String,
        root_digest: String,
    },
    RelocationAmbiguous {
        generation_digest: String,
        root_digest: String,
        candidates: Vec<String>,
    },
}
