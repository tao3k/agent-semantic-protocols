// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Project-wide topology identity, resident index, and admission boundary.
#![forbid(unsafe_code)]

mod agent_org_topology_overlay;
mod org_projection_port;
mod project_topology_library;
mod project_topology_manifest;
mod runtime_project_topology_attachment;
mod topology_index;

pub use agent_org_topology_overlay::{
    AGENT_ORG_TOPOLOGY_OVERLAY_SCHEMA_ID, AGENT_ORG_TOPOLOGY_OVERLAY_SCHEMA_VERSION,
    AgentOrgTopologyOverlay, AgentOrgTopologyOverlayError, AgentOrgTopologyOverlayTerminal,
    AgentTopologyRelationship,
};
pub use agent_semantic_content_identity::ProjectWorkspaceBinding;
pub use org_projection_port::{
    OrgDocumentProjection, OrgDocumentProjectionPort, OrgOverlayDocumentProjection,
    OrgProjectionPortError, OrgPropertyEntry, OrgSectionProjection,
};
pub use project_topology_library::{
    PROJECT_TOPOLOGY_LIBRARY_SCHEMA_ID, PROJECT_TOPOLOGY_LIBRARY_SCHEMA_VERSION,
    ProjectTopologyDisplayRelationship, ProjectTopologyLibrary, ProjectTopologyLibraryError,
};
pub use project_topology_manifest::{
    PROJECT_TOPOLOGY_MANIFEST_PATH, ProjectTopologyManifest, ProjectTopologyManifestError,
};
pub use runtime_project_topology_attachment::{
    RUNTIME_PROJECT_TOPOLOGY_ATTACHMENT_SCHEMA_ID,
    RUNTIME_PROJECT_TOPOLOGY_ATTACHMENT_SCHEMA_VERSION, RuntimeProjectTopologyAttachment,
    RuntimeProjectTopologyAttachmentCandidate, RuntimeProjectTopologyAttachmentError,
};
pub use topology_index::{
    RankedTextTopologySelectorHitV1, TopologyAnchorHitV1, TopologyHitV1, TopologyIndexV1,
    TopologyNodeKindV1, TopologyNodeV1, TopologyOwnerQueryV1, TopologyOwnerV1,
    ranked_text_topology_selector_carrier, topology_feature_terms, topology_navigation_features,
};
