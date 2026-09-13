// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Persisted canonical materialization data and its validated admission wrapper.

use serde::{Deserialize, Serialize};

use super::observability::{
    prepare_canonical_index, record_canonical_materialization_observations,
};
use crate::runtime_server_workspace::{
    WorkspaceAuxiliaryOwnerSnapshot, WorkspaceOwnerSnapshot, memory_backend::WorkspaceMemoryIndex,
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceCanonicalMaterialization {
    pub schema_id: String,
    pub schema_version: String,
    pub workspace_identity: String,
    pub project_root: String,
    pub workspace_snapshot: agent_semantic_content_identity::WorkspaceSnapshot,
    pub source_snapshot: agent_semantic_content_identity::SourceSnapshotEvidence,
    pub workspace_generation: agent_semantic_content_identity::workspace_generation_evidence::WorkspaceGenerationEvidenceV1,
    pub provider_schema_digest: String,
    pub import_digest: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub runtime_provider_execution_binding:
        Option<agent_semantic_artifacts::runtime_provider_execution_binding::RuntimeProviderExecutionBinding>,
    pub selector_set_digest: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content_search_generation: Option<agent_semantic_search::ContentSearchGenerationReceipt>,
    pub projection_capability: crate::active_generation_projection_capability::ActiveGenerationProjectionCapabilityManifest,
    pub workspace_source_scope_generation: String,
    pub project_resolutions: Vec<agent_semantic_content_identity::AdmittedProjectResolution>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub auxiliary_owners: Vec<WorkspaceAuxiliaryOwnerSnapshot>,
    pub relations: Vec<crate::ClientDbSourceIndexOwnedRelation>,
    pub file_count: u32,
    pub root_depth: [u8; 2],
    pub owners: Vec<WorkspaceOwnerSnapshot>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[expect(
    clippy::large_enum_variant,
    reason = "the V1 load result returns the admitted immutable materialization by value"
)]
pub enum WorkspaceCanonicalMaterializationLoad {
    Ready(WorkspaceCanonicalMaterialization),
    Missing,
    Incompatible { reason: String },
}

#[derive(Debug)]
pub struct ValidatedWorkspaceCanonicalMaterialization {
    materialization: WorkspaceCanonicalMaterialization,
    index: std::sync::Arc<WorkspaceMemoryIndex>,
}

impl ValidatedWorkspaceCanonicalMaterialization {
    pub fn new(
        materialization: WorkspaceCanonicalMaterialization,
        workspace_identity: &str,
    ) -> Result<Self, String> {
        materialization.validate_persisted(workspace_identity)?;
        let (index, prepare_index_elapsed_micros) = prepare_canonical_index(&materialization);
        record_canonical_materialization_observations(
            &materialization,
            prepare_index_elapsed_micros,
        );
        Ok(Self {
            materialization,
            index,
        })
    }

    pub(crate) fn into_parts(
        self,
    ) -> (
        WorkspaceCanonicalMaterialization,
        std::sync::Arc<WorkspaceMemoryIndex>,
    ) {
        (self.materialization, self.index)
    }

    pub fn as_materialization(&self) -> &WorkspaceCanonicalMaterialization {
        &self.materialization
    }
}
