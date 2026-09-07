// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Project, repository, and workspace binding identities owned by Artifacts.

use std::path::Path;
use std::path::PathBuf;

use crate::blake3_content_digest::Blake3ContentDigest;
use serde::Deserialize;
use serde::Serialize;

pub const PROJECT_BINDING_SCHEMA_ID: &str = "agent.semantic-protocols.state-home-project-binding";
pub const PROJECT_BINDING_SCHEMA_VERSION: u32 = 1;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HostProjectReference {
    pub platform: String,
    pub project_id: String,
    pub project_kind: String,
    pub host_id: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RepoIdentity {
    pub digest: Blake3ContentDigest,
    pub basis: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceIdentity {
    pub digest: Blake3ContentDigest,
    pub repo_digest: Blake3ContentDigest,
    pub canonical_root: PathBuf,
    pub private_git_dir: Option<PathBuf>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectBinding {
    pub schema_id: String,
    pub schema_version: u32,
    pub host_project: Option<HostProjectReference>,
    pub repo: RepoIdentity,
    pub workspace: WorkspaceIdentity,
    pub binding_digest: Blake3ContentDigest,
}

impl ProjectBinding {
    pub fn resolve(
        host_project: Option<HostProjectReference>,
        repo_basis: impl Into<String>,
        workspace_root: impl AsRef<Path>,
    ) -> Result<Self, String> {
        Self::resolve_with_private_git_dir(host_project, repo_basis, workspace_root, None::<&Path>)
    }

    pub fn resolve_with_private_git_dir(
        host_project: Option<HostProjectReference>,
        repo_basis: impl Into<String>,
        workspace_root: impl AsRef<Path>,
        private_git_dir: Option<impl AsRef<Path>>,
    ) -> Result<Self, String> {
        let repo_basis = non_empty("repo basis", repo_basis.into())?;
        let canonical_root = workspace_root
            .as_ref()
            .canonicalize()
            .unwrap_or_else(|_| workspace_root.as_ref().to_path_buf());
        if canonical_root.as_os_str().is_empty() {
            return Err("workspace root must not be empty".to_string());
        }
        if let Some(reference) = &host_project {
            reference.validate()?;
        }
        let private_git_dir = private_git_dir.map(|path| {
            path.as_ref()
                .canonicalize()
                .unwrap_or_else(|_| path.as_ref().to_path_buf())
        });

        let repo_digest = digest_json(&serde_json::json!({
            "domain": "asp-state-home-repo-identity",
            "basis": repo_basis,
        }))?;
        let workspace_digest = digest_json(&serde_json::json!({
            "domain": "asp-state-home-workspace-identity",
            "repoDigest": repo_digest,
            "canonicalRoot": canonical_root,
            "privateGitDir": private_git_dir,
        }))?;
        let binding_digest = digest_json(&serde_json::json!({
            "domain": "asp-state-home-project-binding",
            "hostProject": host_project,
            "repoDigest": repo_digest,
            "workspaceDigest": workspace_digest,
        }))?;

        Ok(Self {
            schema_id: PROJECT_BINDING_SCHEMA_ID.to_string(),
            schema_version: PROJECT_BINDING_SCHEMA_VERSION,
            host_project,
            repo: RepoIdentity {
                digest: repo_digest.clone(),
                basis: repo_basis,
            },
            workspace: WorkspaceIdentity {
                digest: workspace_digest,
                repo_digest,
                canonical_root,
                private_git_dir,
            },
            binding_digest,
        })
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.schema_id != PROJECT_BINDING_SCHEMA_ID
            || self.schema_version != PROJECT_BINDING_SCHEMA_VERSION
        {
            return Err("project binding schema identity mismatch".to_string());
        }
        let expected = Self::resolve_with_private_git_dir(
            self.host_project.clone(),
            self.repo.basis.clone(),
            &self.workspace.canonical_root,
            self.workspace.private_git_dir.as_deref(),
        )?;
        if &expected != self {
            return Err("project binding digest or identity mismatch".to_string());
        }
        Ok(())
    }
}

impl HostProjectReference {
    fn validate(&self) -> Result<(), String> {
        non_empty("host platform", self.platform.clone())?;
        non_empty("host project id", self.project_id.clone())?;
        non_empty("host project kind", self.project_kind.clone())?;
        if self.host_id.as_deref().is_some_and(str::is_empty) {
            return Err("host id must not be empty when present".to_string());
        }
        Ok(())
    }
}

fn non_empty(label: &str, value: String) -> Result<String, String> {
    if value.is_empty() {
        Err(format!("{label} must not be empty"))
    } else {
        Ok(value)
    }
}

fn digest_json(value: &serde_json::Value) -> Result<Blake3ContentDigest, String> {
    let bytes = serde_json::to_vec(value).map_err(|error| format!("encode identity: {error}"))?;
    Ok(Blake3ContentDigest::from_bytes(&bytes))
}
