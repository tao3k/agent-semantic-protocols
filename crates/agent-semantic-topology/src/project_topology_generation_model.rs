// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Pure source and candidate values for Project Topology generation.

use std::collections::BTreeMap;
use std::sync::Arc;

use agent_semantic_content_identity::{CanonicalItemSelector, ProjectWorkspaceBinding};
use serde_json::Value;

use crate::project_topology_generation_error::{
    ProjectTopologyGenerationBuildError, error, require_digest, validate_identifier,
};
use crate::{
    ProjectTopologyDirectEdge, ProjectTopologyInferenceProgram, ProjectTopologyLibrary,
    ProjectTopologyLibraryError,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectTopologySourceNode {
    pub(super) id: String,
    pub(super) locator: ProjectTopologySourceNodeLocator,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum ProjectTopologySourceNodeLocator {
    Selector(CanonicalItemSelector),
    Owner {
        language_id: String,
        owner_path: String,
    },
}

impl ProjectTopologySourceNode {
    pub fn new(
        id: impl Into<String>,
        selector: impl Into<String>,
    ) -> Result<Self, ProjectTopologyGenerationBuildError> {
        let id = id.into();
        validate_identifier(&id, "node identity")?;
        let selector = CanonicalItemSelector::parse(selector.into()).map_err(|cause| {
            error(
                "topology-generation-selector-invalid",
                format!("cannot decode parser-owned selector: {cause}"),
            )
        })?;
        Ok(Self {
            id,
            locator: ProjectTopologySourceNodeLocator::Selector(selector),
        })
    }

    pub fn new_owner(
        id: impl Into<String>,
        language_id: impl Into<String>,
        owner_path: impl Into<String>,
    ) -> Result<Self, ProjectTopologyGenerationBuildError> {
        let id = id.into();
        validate_identifier(&id, "node identity")?;
        let language_id = language_id.into();
        if language_id.is_empty()
            || !language_id.bytes().all(|byte| {
                byte.is_ascii_lowercase()
                    || byte.is_ascii_digit()
                    || matches!(byte, b'-' | b'_' | b'.' | b'+')
            })
        {
            return Err(error(
                "topology-generation-owner-language-invalid",
                "owner locator language must be canonical",
            ));
        }
        let owner_path = owner_path.into();
        validate_owner_path(&owner_path)?;
        Ok(Self {
            id,
            locator: ProjectTopologySourceNodeLocator::Owner {
                language_id,
                owner_path,
            },
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectTopologySourceSegment {
    pub(super) owner_path: String,
    pub(super) content_digest: String,
    pub(super) nodes: Vec<ProjectTopologySourceNode>,
    pub(super) edges: Vec<ProjectTopologyDirectEdge>,
}

impl ProjectTopologySourceSegment {
    pub fn new(
        owner_path: impl Into<String>,
        content_digest: impl Into<String>,
        mut nodes: Vec<ProjectTopologySourceNode>,
        mut edges: Vec<ProjectTopologyDirectEdge>,
    ) -> Result<Self, ProjectTopologyGenerationBuildError> {
        let owner_path = owner_path.into();
        validate_owner_path(&owner_path)?;
        let content_digest = content_digest.into();
        require_digest(&content_digest, "content digest")?;
        nodes.sort_by(|left, right| left.id.cmp(&right.id));
        edges.sort_by(|left, right| left.id().cmp(right.id()));
        ensure_unique(nodes.iter().map(|node| node.id.as_str()), "node")?;
        ensure_unique(edges.iter().map(ProjectTopologyDirectEdge::id), "edge")?;
        Ok(Self {
            owner_path,
            content_digest,
            nodes,
            edges,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectTopologyGenerationIdentity {
    pub(super) project_workspace: ProjectWorkspaceBinding,
    pub(super) source_generation_digest: String,
    pub(super) provider_catalog_digest: String,
    pub(super) inference_program: Arc<ProjectTopologyInferenceProgram>,
    pub(super) provider_grammar_digest: String,
    pub(super) resolver_digest: String,
    pub(super) topology_schema_digest: String,
}

impl ProjectTopologyGenerationIdentity {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        project_workspace: ProjectWorkspaceBinding,
        source_generation_digest: String,
        provider_catalog_digest: String,
        inference_program: Arc<ProjectTopologyInferenceProgram>,
        provider_grammar_digest: String,
        resolver_digest: String,
        topology_schema_digest: String,
    ) -> Result<Self, ProjectTopologyGenerationBuildError> {
        project_workspace
            .validate()
            .map_err(|cause| error("topology-generation-workspace-invalid", cause.to_string()))?;
        require_digest(inference_program.digest(), "inference program")?;
        for (name, digest) in [
            ("source generation", &source_generation_digest),
            ("provider catalog", &provider_catalog_digest),
            ("provider grammar", &provider_grammar_digest),
            ("resolver", &resolver_digest),
            ("topology schema", &topology_schema_digest),
        ] {
            require_digest(digest, name)?;
        }
        Ok(Self {
            project_workspace,
            source_generation_digest,
            provider_catalog_digest,
            inference_program,
            provider_grammar_digest,
            resolver_digest,
            topology_schema_digest,
        })
    }
}

#[derive(Clone, Debug)]
pub struct ProjectTopologyGenerationCandidate {
    pub(super) packet: Value,
    pub(super) rebuild_receipt: Value,
    pub(super) rebuild_receipt_id: String,
    pub(super) project_workspace: ProjectWorkspaceBinding,
}

impl ProjectTopologyGenerationCandidate {
    pub fn packet(&self) -> &Value {
        &self.packet
    }

    pub fn rebuild_receipt(&self) -> &Value {
        &self.rebuild_receipt
    }

    pub fn rebuild_receipt_id(&self) -> &str {
        &self.rebuild_receipt_id
    }

    pub fn admit(
        self,
        independently_admitted_receipts: &BTreeMap<String, Value>,
    ) -> Result<ProjectTopologyLibrary, ProjectTopologyLibraryError> {
        ProjectTopologyLibrary::admit_with_receipts(
            self.packet,
            &self.project_workspace,
            independently_admitted_receipts,
        )
    }
}

pub(super) fn validate_owner_path(
    owner_path: &str,
) -> Result<(), ProjectTopologyGenerationBuildError> {
    if owner_path.is_empty()
        || owner_path.starts_with('/')
        || owner_path.split('/').any(|part| {
            part.is_empty()
                || part == "."
                || part == ".."
                || part.contains('\\')
                || part.chars().any(char::is_control)
        })
    {
        return Err(error(
            "topology-generation-owner-path-invalid",
            "owner path must be normalized and project-relative",
        ));
    }
    Ok(())
}

pub(super) fn ensure_unique<'a>(
    values: impl IntoIterator<Item = &'a str>,
    kind: &str,
) -> Result<(), ProjectTopologyGenerationBuildError> {
    let mut seen = std::collections::BTreeSet::new();
    for value in values {
        if !seen.insert(value) {
            return Err(error(
                "topology-generation-identity-duplicate",
                format!("duplicate {kind} identity: {value}"),
            ));
        }
    }
    Ok(())
}
