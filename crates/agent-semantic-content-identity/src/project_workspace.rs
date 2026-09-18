// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Durable GitOps Project Workspace identity and repository containment boundary.

use std::fmt;

use serde::{Deserialize, Serialize};

pub const PROJECT_WORKSPACE_BINDING_SCHEMA_ID: &str =
    "agent.semantic-protocols.project-workspace-binding";
pub const PROJECT_WORKSPACE_BINDING_SCHEMA_VERSION: &str = "1";
pub const HOST_WORKSPACE_INITIALIZATION_BINDING_SCHEMA_ID: &str =
    "agent.semantic-protocols.host-workspace-initialization-binding";
pub const HOST_WORKSPACE_INITIALIZATION_BINDING_SCHEMA_VERSION: &str = "1";

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectWorkspaceBinding {
    schema_id: String,
    schema_version: String,
    project_workspace_identity: String,
    workspace_root_path: String,
    portability: String,
    repository_aliases: Vec<String>,
}

/// One Host-admitted local worktree bound to one parser-owned Project Workspace.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct HostWorkspaceInitializationBinding {
    schema_id: String,
    schema_version: String,
    project_workspace: ProjectWorkspaceBinding,
    worktree_instance_id: String,
}

impl HostWorkspaceInitializationBinding {
    pub fn new(
        project_workspace: ProjectWorkspaceBinding,
        worktree_instance_id: impl Into<String>,
    ) -> Result<Self, ProjectWorkspaceBindingError> {
        let binding = Self {
            schema_id: HOST_WORKSPACE_INITIALIZATION_BINDING_SCHEMA_ID.to_owned(),
            schema_version: HOST_WORKSPACE_INITIALIZATION_BINDING_SCHEMA_VERSION.to_owned(),
            project_workspace,
            worktree_instance_id: worktree_instance_id.into(),
        };
        binding.validate()?;
        Ok(binding)
    }

    pub fn validate(&self) -> Result<(), ProjectWorkspaceBindingError> {
        if self.schema_id != HOST_WORKSPACE_INITIALIZATION_BINDING_SCHEMA_ID
            || self.schema_version != HOST_WORKSPACE_INITIALIZATION_BINDING_SCHEMA_VERSION
        {
            return invalid(
                "host-workspace-initialization-schema-mismatch",
                "Host workspace initialization binding schema identity is not current",
            );
        }
        self.project_workspace.validate()?;
        if self.worktree_instance_id.trim().is_empty() {
            return invalid(
                "host-worktree-instance-missing",
                "Host workspace initialization requires an independently supplied worktree instance identity",
            );
        }
        Ok(())
    }

    pub fn admit_exact(&self, candidate: &Self) -> Result<(), ProjectWorkspaceBindingError> {
        self.validate()?;
        candidate.validate()?;
        if self != candidate {
            return invalid(
                "host-workspace-initialization-binding-mismatch",
                "Host workspace initialization binding differs from the admitted product",
            );
        }
        Ok(())
    }

    pub fn project_workspace(&self) -> &ProjectWorkspaceBinding {
        &self.project_workspace
    }

    pub fn worktree_instance_id(&self) -> &str {
        &self.worktree_instance_id
    }
}

impl ProjectWorkspaceBinding {
    pub fn new(
        project_workspace_identity: impl Into<String>,
        workspace_root_path: impl Into<String>,
        portability: impl Into<String>,
        repository_aliases: Vec<String>,
    ) -> Result<Self, ProjectWorkspaceBindingError> {
        let binding = Self {
            schema_id: PROJECT_WORKSPACE_BINDING_SCHEMA_ID.to_owned(),
            schema_version: PROJECT_WORKSPACE_BINDING_SCHEMA_VERSION.to_owned(),
            project_workspace_identity: project_workspace_identity.into(),
            workspace_root_path: workspace_root_path.into(),
            portability: portability.into(),
            repository_aliases,
        };
        binding.validate()?;
        Ok(binding)
    }

    pub fn validate(&self) -> Result<(), ProjectWorkspaceBindingError> {
        if self.schema_id != PROJECT_WORKSPACE_BINDING_SCHEMA_ID
            || self.schema_version != PROJECT_WORKSPACE_BINDING_SCHEMA_VERSION
        {
            return invalid(
                "topology-project-workspace-schema-mismatch",
                "Project Workspace binding schema identity is not current",
            );
        }
        validate_project_workspace_identity(&self.project_workspace_identity, &self.portability)?;
        validate_workspace_root_path(&self.workspace_root_path)?;
        if self.portability == "local-only" && !self.repository_aliases.is_empty() {
            return invalid(
                "topology-local-workspace-alias-unsupported",
                "a local-only workspace cannot claim durable repository aliases",
            );
        }
        for (index, alias) in self.repository_aliases.iter().enumerate() {
            if !is_durable_repository_locator(alias) {
                return invalid(
                    "topology-project-workspace-alias-invalid",
                    "repository aliases must be durable Git locators",
                );
            }
            if index > 0 && self.repository_aliases[index - 1] >= *alias {
                return invalid(
                    "topology-project-workspace-alias-invalid",
                    "repository aliases must be unique and lexicographically ordered",
                );
            }
        }
        Ok(())
    }

    pub fn project_workspace_identity(&self) -> &str {
        &self.project_workspace_identity
    }

    pub fn workspace_root_path(&self) -> &str {
        &self.workspace_root_path
    }

    pub fn portability(&self) -> &str {
        &self.portability
    }

    pub fn repository_aliases(&self) -> &[String] {
        &self.repository_aliases
    }
}

fn validate_project_workspace_identity(
    identity: &str,
    portability: &str,
) -> Result<(), ProjectWorkspaceBindingError> {
    let (repository_locator, workspace_key) = identity
        .split_once("#workspace/")
        .ok_or_else(|| error("topology-project-workspace-identity-invalid", identity))?;
    if workspace_key.is_empty()
        || workspace_key.split('/').any(|component| {
            component.is_empty()
                || !component.starts_with(|character: char| character.is_ascii_lowercase())
                || !component.chars().all(|character| {
                    character.is_ascii_lowercase()
                        || character.is_ascii_digit()
                        || matches!(character, '.' | '_' | '-')
                })
        })
    {
        return invalid(
            "topology-project-workspace-identity-invalid",
            "logical workspace key is not canonical",
        );
    }
    let durable = is_durable_repository_locator(repository_locator);
    let local = repository_locator.starts_with("git+file:///");
    match portability {
        "cross-machine" if durable => Ok(()),
        "local-only" if local => Ok(()),
        "cross-machine" | "local-only" => invalid(
            "topology-project-workspace-portability-mismatch",
            "project workspace locator does not match its portability",
        ),
        _ => invalid(
            "topology-project-workspace-portability-unsupported",
            "project workspace portability is unsupported",
        ),
    }
}

fn validate_workspace_root_path(path: &str) -> Result<(), ProjectWorkspaceBindingError> {
    if path == "." {
        return Ok(());
    }
    if path.starts_with('/')
        || path.contains('\\')
        || path.contains('#')
        || path.chars().any(char::is_whitespace)
        || path
            .split('/')
            .any(|component| component.is_empty() || matches!(component, "." | ".."))
    {
        return invalid(
            "topology-workspace-root-path-invalid",
            "workspace root must be a normalized repository-relative path",
        );
    }
    Ok(())
}

fn is_durable_repository_locator(locator: &str) -> bool {
    (locator.starts_with("git+https://") || locator.starts_with("git+ssh://"))
        && locator.ends_with(".git")
        && !locator.contains('#')
        && !locator.chars().any(char::is_whitespace)
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectWorkspaceBindingError {
    reason_kind: &'static str,
    message: String,
}

impl ProjectWorkspaceBindingError {
    pub fn reason_kind(&self) -> &'static str {
        self.reason_kind
    }
}

impl fmt::Display for ProjectWorkspaceBindingError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}: {}", self.reason_kind, self.message)
    }
}

impl std::error::Error for ProjectWorkspaceBindingError {}

fn error(reason_kind: &'static str, message: impl Into<String>) -> ProjectWorkspaceBindingError {
    ProjectWorkspaceBindingError {
        reason_kind,
        message: message.into(),
    }
}

fn invalid<T>(
    reason_kind: &'static str,
    message: impl Into<String>,
) -> Result<T, ProjectWorkspaceBindingError> {
    Err(error(reason_kind, message))
}
