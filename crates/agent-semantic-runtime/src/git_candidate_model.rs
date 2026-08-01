use std::path::{Path, PathBuf};

use super::{
    GitWorkspaceFileScopeError, canonicalize_if_possible, discover_repository_candidate_snapshot,
};

#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RepositoryCandidateSnapshot {
    pub schema_id: String,
    pub schema_version: String,
    pub mode: RepositoryCandidateMode,
    pub repository_identity: RepositoryIdentity,
    pub worktree_identity: WorktreeIdentity,
    pub candidate_scope: RepositoryCandidateScope,
    pub candidate_generation: RepositoryCandidateGeneration,
    pub candidates: Vec<RepositoryCandidate>,
    pub policy_overlay_digest: String,
    pub policy_exclusions: Vec<RepositoryCandidatePolicyExclusion>,
    pub metrics: RepositoryCandidateMetrics,
}

impl RepositoryCandidateSnapshot {
    pub fn scoped_to_project_root(
        &self,
        project_root: &Path,
    ) -> Result<Self, GitWorkspaceFileScopeError> {
        let project_root = canonicalize_if_possible(project_root);
        let relative_scope = project_root
            .strip_prefix(&self.candidate_scope.project_root)
            .map_err(
                |_| GitWorkspaceFileScopeError::ProjectOutsideCandidateScope {
                    project_root: project_root.clone(),
                    candidate_root: self.candidate_scope.project_root.clone(),
                },
            )?
            .to_path_buf();
        if relative_scope.as_os_str().is_empty() {
            return Ok(self.clone());
        }
        discover_repository_candidate_snapshot(&project_root)?
            .ok_or(GitWorkspaceFileScopeError::ProjectRepositoryUnavailable { project_root })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum RepositoryCandidateMode {
    Git,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RepositoryIdentity {
    pub repository_id: String,
    pub identity_basis: String,
    pub git_common_dir: PathBuf,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub remote_url: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorktreeIdentity {
    pub worktree_id: String,
    pub worktree_root: PathBuf,
    pub git_dir: PathBuf,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub head_id: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RepositoryCandidateScope {
    pub project_root: PathBuf,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RepositoryCandidateGeneration {
    pub algorithm: String,
    pub digest: String,
    pub authorities: Vec<RepositoryCandidateAuthority>,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RepositoryCandidate {
    pub path: PathBuf,
    pub state: RepositoryCandidateState,
    pub authority: RepositoryCandidateAuthority,
}

#[derive(
    Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, serde::Deserialize, serde::Serialize,
)]
#[serde(rename_all = "kebab-case")]
pub enum RepositoryCandidateState {
    Tracked,
    Untracked,
}

#[derive(
    Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, serde::Deserialize, serde::Serialize,
)]
#[serde(rename_all = "kebab-case")]
pub enum RepositoryCandidateAuthority {
    GitIndex,
    GitWorktree,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RepositoryCandidatePolicyExclusion {
    pub path: PathBuf,
    pub authority: String,
    pub reason_kind: String,
    pub matched_value: String,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RepositoryCandidateMetrics {
    pub index_entry_count: usize,
    pub worktree_addition_count: usize,
    pub candidate_count: usize,
    pub policy_exclusion_count: usize,
    pub full_workspace_reads: usize,
    pub full_merkle_rebuilds: usize,
    pub direct_db_opens: usize,
}
