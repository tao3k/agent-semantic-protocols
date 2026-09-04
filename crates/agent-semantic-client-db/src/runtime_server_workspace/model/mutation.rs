//! Generation mutation and exact-selector overlay contracts.

use serde::{Deserialize, Serialize};

use super::{ExactProjectionKind, RuntimeProjectionScope, WorkspaceOwnerSnapshot, validate_owners};

pub const WORKSPACE_GENERATION_DELTA_SCHEMA_ID: &str =
    "agent.semantic-protocols.workspace-generation-delta.v2";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceGenerationDelta {
    pub schema_id: String,
    pub schema_version: String,
    pub base_generation_digest: String,
    pub owners: Vec<WorkspaceOwnerSnapshot>,
    pub tombstones: Vec<String>,
    pub relations: Vec<crate::ClientDbSourceIndexOwnedRelation>,
}

impl WorkspaceGenerationDelta {
    pub fn validate(&self) -> Result<(), String> {
        self.validate_identity_and_payload()?;
        let tombstones = self.validate_tombstones()?;
        self.validate_owner_tombstone_disjoint(&tombstones)?;
        self.validate_relation_ownership()
    }

    fn validate_identity_and_payload(&self) -> Result<(), String> {
        if self.schema_id != WORKSPACE_GENERATION_DELTA_SCHEMA_ID || self.schema_version != "2" {
            return Err("workspace generation delta schema identity mismatch".to_owned());
        }
        if !self.base_generation_digest.starts_with("blake3-256:") {
            return Err("workspace generation delta base digest is invalid".to_owned());
        }
        if self.owners.is_empty() && self.tombstones.is_empty() {
            return Err("workspace generation delta must contain at least one mutation".to_owned());
        }
        validate_owners(&self.owners)?;
        Ok(())
    }

    fn validate_relation_ownership(&self) -> Result<(), String> {
        let changed_owners = self
            .owners
            .iter()
            .map(|owner| owner.owner_path.as_str())
            .collect::<std::collections::BTreeSet<_>>();
        for owned in &self.relations {
            owned.relation.validate()?;
            if !changed_owners.contains(owned.owner_path.as_str()) {
                return Err(format!(
                    "workspace generation delta relation is outside changed owner membership: {}",
                    owned.owner_path.as_str()
                ));
            }
        }
        Ok(())
    }

    fn validate_tombstones(&self) -> Result<std::collections::HashSet<&str>, String> {
        let mut tombstones = std::collections::HashSet::with_capacity(self.tombstones.len());
        for owner_path in &self.tombstones {
            if owner_path.trim().is_empty() || !tombstones.insert(owner_path.as_str()) {
                return Err("workspace generation delta tombstones must be unique paths".to_owned());
            }
        }
        Ok(tombstones)
    }

    fn validate_owner_tombstone_disjoint(
        &self,
        tombstones: &std::collections::HashSet<&str>,
    ) -> Result<(), String> {
        if self
            .owners
            .iter()
            .any(|owner| tombstones.contains(owner.owner_path.as_str()))
        {
            return Err(
                "workspace generation delta cannot upsert and tombstone the same owner".to_owned(),
            );
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
