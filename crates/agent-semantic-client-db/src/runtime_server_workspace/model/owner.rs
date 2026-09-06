// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-only

//! Owner-local snapshots and compact read results.

use serde::{Deserialize, Serialize};

use super::core::ExactProjectionKind;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceSelectorSnapshot {
    pub selector: String,
    pub byte_start: usize,
    pub byte_end: usize,
    #[serde(default)]
    pub query_keys: Vec<String>,
    pub derived_projections: Vec<WorkspaceDerivedProjectionSnapshot>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceDerivedProjectionSnapshot {
    pub projection_kind: ExactProjectionKind,
    pub bytes: Vec<u8>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub evidence_context: Option<
        agent_semantic_content_identity::projection_evidence_context::ProjectionEvidenceContext,
    >,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceOwnerSnapshot {
    pub owner_path: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub authority: Option<agent_semantic_search::ResidentSearchAuthority>,
    pub content_digest: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub native_syntax_diagnostic: Option<agent_semantic_search::NativeSyntaxDiagnostic>,
    pub bytes: Vec<u8>,
    pub selectors: Vec<WorkspaceSelectorSnapshot>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspaceOwnerProjection {
    pub owner: WorkspaceOwnerSnapshot,
    pub relations: Vec<crate::ClientDbSourceIndexOwnedRelation>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspaceOwnerSearchSeedSnapshot {
    pub selector: String,
    pub byte_start: usize,
    pub byte_end: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspaceOwnerSearchSnapshot {
    pub owner_path: String,
    pub content_digest: String,
    pub candidate_count: usize,
    pub selectors: Vec<WorkspaceOwnerSearchSeedSnapshot>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WorkspaceRuntimeOwnerSearchRead {
    Owner {
        generation_digest: String,
        root_digest: String,
        owner: WorkspaceOwnerSearchSnapshot,
    },
    OwnerMissing {
        generation_digest: String,
        root_digest: String,
    },
}
