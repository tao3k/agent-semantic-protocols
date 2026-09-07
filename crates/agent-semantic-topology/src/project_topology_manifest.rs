// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Native Org projection for the Git-tracked Project Topology manifest.

use std::collections::BTreeMap;
use std::fmt;
use std::fs;
use std::path::Path;

use agent_semantic_content_identity::{ProjectWorkspaceBinding, ProjectWorkspaceBindingError};
use orgize::Org;
use orgize::rowan::ast::AstNode;
use orgize::syntax_ast::{Headline, PropertyDrawer};

pub const PROJECT_TOPOLOGY_MANIFEST_PATH: &str = ".agents/asp/topology/manifest.org";

const PROJECT_TOPOLOGY_CONTRACT: &str = "project.topology-program.v1.org";
const PROGRAM_ID: &str = "TOPOLOGY_PROGRAM_ID";
const PROJECT_WORKSPACE_IDENTITY: &str = "PROJECT_WORKSPACE_IDENTITY";
const WORKSPACE_ROOT_PATH: &str = "WORKSPACE_ROOT_PATH";
const PORTABILITY: &str = "PORTABILITY";
const REPOSITORY_ALIASES: &str = "REPOSITORY_ALIASES";

/// Parser-owned projection of one canonical Project Topology manifest.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectTopologyManifest {
    project_workspace: ProjectWorkspaceBinding,
}

impl ProjectTopologyManifest {
    pub fn load_from_project_root(
        project_root: &Path,
    ) -> Result<Self, ProjectTopologyManifestError> {
        let path = project_root.join(PROJECT_TOPOLOGY_MANIFEST_PATH);
        let metadata = fs::symlink_metadata(&path).map_err(|io_error| {
            error(
                "topology-manifest-unavailable",
                format!("cannot read {}: {io_error}", path.display()),
            )
        })?;
        if metadata.file_type().is_symlink() || !metadata.is_file() {
            return invalid(
                "topology-manifest-path-invalid",
                "canonical Project Topology manifest must be a regular file",
            );
        }
        let source = fs::read_to_string(&path).map_err(|io_error| {
            error(
                "topology-manifest-unavailable",
                format!("cannot read {}: {io_error}", path.display()),
            )
        })?;
        Self::parse_org(&source)
    }

    pub fn parse_org(source: &str) -> Result<Self, ProjectTopologyManifestError> {
        let org = Org::parse(source);
        let contract_properties = org.syntax_document().properties().ok_or_else(|| {
            error(
                "topology-manifest-contract-mismatch",
                "manifest has no CONTRACT_ORG property",
            )
        })?;
        let contract = unique_property(&contract_properties, "CONTRACT_ORG")?.ok_or_else(|| {
            error(
                "topology-manifest-contract-mismatch",
                "manifest has no CONTRACT_ORG property",
            )
        })?;
        if !contract_reference_is_canonical(&contract) {
            return invalid(
                "topology-manifest-contract-mismatch",
                "manifest is not governed by project.topology-program.v1",
            );
        }

        let declarations = org
            .syntax_document()
            .syntax()
            .descendants()
            .filter_map(Headline::cast)
            .filter(|headline| headline.level() == 1)
            .filter_map(|headline| {
                headline
                    .properties()
                    .and_then(|properties| manifest_declaration(&properties).then_some(properties))
            })
            .collect::<Vec<_>>();
        let [properties] = declarations.as_slice() else {
            let reason = if declarations.is_empty() {
                "topology-manifest-missing"
            } else {
                "topology-manifest-ambiguous"
            };
            return invalid(
                reason,
                format!(
                    "expected one level-one Project Topology declaration, observed {}",
                    declarations.len()
                ),
            );
        };

        let fields = unique_properties(properties)?;
        let project_workspace_identity = required_property(&fields, PROJECT_WORKSPACE_IDENTITY)?;
        let workspace_root_path = required_property(&fields, WORKSPACE_ROOT_PATH)?;
        let portability = required_property(&fields, PORTABILITY)?;
        let aliases_source = required_property(&fields, REPOSITORY_ALIASES)?;
        let repository_aliases =
            serde_json::from_str::<Vec<String>>(aliases_source).map_err(|_| {
                error(
                    "topology-manifest-aliases-invalid",
                    "REPOSITORY_ALIASES must be a JSON array of repository locators",
                )
            })?;
        if repository_aliases.windows(2).any(|pair| pair[0] >= pair[1]) {
            return invalid(
                "topology-manifest-aliases-invalid",
                "REPOSITORY_ALIASES must be unique and lexicographically ordered",
            );
        }

        let project_workspace = ProjectWorkspaceBinding::new(
            project_workspace_identity,
            workspace_root_path,
            portability,
            repository_aliases,
        )
        .map_err(ProjectTopologyManifestError::from_project_workspace)?;

        Ok(Self { project_workspace })
    }

    pub fn project_workspace(&self) -> &ProjectWorkspaceBinding {
        &self.project_workspace
    }
}

fn manifest_declaration(properties: &PropertyDrawer) -> bool {
    properties.get(PROGRAM_ID).is_some()
}

fn unique_properties(
    properties: &PropertyDrawer,
) -> Result<BTreeMap<String, String>, ProjectTopologyManifestError> {
    let mut result = BTreeMap::new();
    for (key, value) in properties.iter() {
        let key = key.to_string();
        if result.insert(key.clone(), value.to_string()).is_some() {
            return invalid(
                "topology-manifest-property-duplicate",
                format!("manifest property {key} is declared more than once"),
            );
        }
    }
    Ok(result)
}

fn unique_property(
    properties: &PropertyDrawer,
    key: &str,
) -> Result<Option<String>, ProjectTopologyManifestError> {
    let values = properties
        .iter()
        .filter_map(|(observed, value)| (observed == key).then(|| value.to_string()))
        .collect::<Vec<_>>();
    match values.as_slice() {
        [] => Ok(None),
        [value] => Ok(Some(value.clone())),
        _ => invalid(
            "topology-manifest-property-duplicate",
            format!("manifest property {key} is declared more than once"),
        ),
    }
}

fn required_property<'a>(
    properties: &'a BTreeMap<String, String>,
    key: &str,
) -> Result<&'a str, ProjectTopologyManifestError> {
    properties
        .get(key)
        .map(String::as_str)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            error(
                "topology-manifest-property-missing",
                format!("manifest property {key} is required"),
            )
        })
}

fn contract_reference_is_canonical(value: &str) -> bool {
    let Some(link) = value
        .strip_prefix("[[")
        .and_then(|value| value.strip_suffix("]]"))
    else {
        return false;
    };
    let Some((target, label)) = link.split_once("][") else {
        return false;
    };
    label == "project.topology-program.v1"
        && (target == PROJECT_TOPOLOGY_CONTRACT
            || target.ends_with(&format!("/{PROJECT_TOPOLOGY_CONTRACT}")))
        && !target.starts_with('/')
        && !target.contains("\\")
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectTopologyManifestError {
    reason_kind: &'static str,
    message: String,
}

impl ProjectTopologyManifestError {
    pub fn reason_kind(&self) -> &'static str {
        self.reason_kind
    }

    fn from_project_workspace(error: ProjectWorkspaceBindingError) -> Self {
        Self {
            reason_kind: error.reason_kind(),
            message: error.to_string(),
        }
    }
}

impl fmt::Display for ProjectTopologyManifestError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}: {}", self.reason_kind, self.message)
    }
}

impl std::error::Error for ProjectTopologyManifestError {}

fn error(reason_kind: &'static str, message: impl Into<String>) -> ProjectTopologyManifestError {
    ProjectTopologyManifestError {
        reason_kind,
        message: message.into(),
    }
}

fn invalid<T>(
    reason_kind: &'static str,
    message: impl Into<String>,
) -> Result<T, ProjectTopologyManifestError> {
    Err(error(reason_kind, message))
}
