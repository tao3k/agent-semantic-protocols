// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Project-wide topology identity and admission boundary.
#![forbid(unsafe_code)]

mod project_topology_closure;
mod project_topology_frontier;
mod project_topology_generation_builder;
mod project_topology_generation_error;
mod project_topology_library;
mod project_topology_manifest;
mod project_topology_program_binding;
mod project_topology_source_program;
mod runtime_project_topology_attachment;

pub use agent_semantic_content_identity::ProjectWorkspaceBinding;
pub use project_topology_closure::{
    ProjectTopologyClosure, ProjectTopologyClosureBuilder, ProjectTopologyClosureError,
    ProjectTopologyClosureLimits, ProjectTopologyDirectEdge, ProjectTopologyInferenceProgram,
    ProjectTopologyInferenceReceipt, ProjectTopologyRelationship,
};
pub use project_topology_frontier::{
    ProjectTopologyExpectedRelation, ProjectTopologyRelationCoverage,
};
pub use project_topology_generation_builder::{
    ProjectTopologyGenerationBuilder, ProjectTopologyGenerationCandidate,
    ProjectTopologyGenerationIdentity, ProjectTopologySourceNode, ProjectTopologySourceSegment,
};
pub use project_topology_generation_error::ProjectTopologyGenerationBuildError;
pub use project_topology_library::{
    PROJECT_TOPOLOGY_LIBRARY_SCHEMA_ID, PROJECT_TOPOLOGY_LIBRARY_SCHEMA_VERSION,
    ProjectTopologyDisplayRelationship, ProjectTopologyLibrary, ProjectTopologyLibraryError,
};
pub use project_topology_manifest::{
    PROJECT_TOPOLOGY_MANIFEST_PATH, ProjectTopologyManifest, ProjectTopologyManifestError,
};
pub use project_topology_program_binding::{
    ProjectTopologyProgramBindingError, admit_project_topology_program_bundle,
};
pub use project_topology_source_program::{
    PROJECT_TOPOLOGY_STANDARD_PROGRAM_RESOURCE, ProjectTopologySourceExpectation,
    ProjectTopologySourceProgram, ProjectTopologySourceProgramError, ProjectTopologySourceRule,
};
pub use runtime_project_topology_attachment::{
    RUNTIME_PROJECT_TOPOLOGY_ATTACHMENT_SCHEMA_ID,
    RUNTIME_PROJECT_TOPOLOGY_ATTACHMENT_SCHEMA_VERSION, RuntimeProjectTopologyAttachment,
    RuntimeProjectTopologyAttachmentCandidate, RuntimeProjectTopologyAttachmentError,
};
