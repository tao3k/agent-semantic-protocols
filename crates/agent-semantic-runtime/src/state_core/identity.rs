//! Repository, workspace, and scope identities.

use crate::git::{GitIdentity, RemoteUrl, canonicalize_if_possible, path_identity};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    fmt,
    path::{Path, PathBuf},
};

/// Stable executable identity for a repository.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct RepoId(pub String);

impl RepoId {
    /// Borrow the stable repository id as a path-safe string.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for RepoId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

/// Stable executable identity for a concrete checkout or worktree.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct WorkspaceId(pub String);

impl WorkspaceId {
    /// Borrow the stable workspace id as a path-safe string.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for WorkspaceId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

/// Stable executable identity for a state scope.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ScopeId(pub String);

impl fmt::Display for ScopeId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

/// Whether a resolved repository identity may own durable State Home data.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum RepoPersistence {
    /// Compatibility value for metadata written before persistence was explicit.
    #[default]
    Legacy,
    /// A Git repository identified by its remote or Git metadata.
    Git,
    /// An explicitly declared non-Git project.
    ExplicitNonGit,
    /// An ordinary path used only as a transient search root.
    EphemeralPath,
}

impl RepoPersistence {
    /// Stable diagnostic and manifest representation.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Legacy => "legacy",
            Self::Git => "git",
            Self::ExplicitNonGit => "explicit-non-git",
            Self::EphemeralPath => "ephemeral-path",
        }
    }

    /// Return whether this identity may be materialized in State Home.
    pub fn is_durable(self) -> bool {
        !matches!(self, Self::EphemeralPath)
    }
}

/// Repository identity and the facts used to derive it.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RepoIdentity {
    pub repo_id: RepoId,
    pub display_name: String,
    pub checkout_root: PathBuf,
    pub git_toplevel: Option<PathBuf>,
    pub git_dir: Option<PathBuf>,
    pub git_common_dir: Option<PathBuf>,
    pub remote_url: Option<RemoteUrl>,
    pub identity_basis: String,
    #[serde(default)]
    pub persistence: RepoPersistence,
}

impl RepoIdentity {
    pub(super) fn from_checkout(git: &GitIdentity, checkout: &CheckoutIdentity) -> Self {
        let remote_basis = git
            .remote_url
            .as_ref()
            .and_then(RemoteUrl::canonical_identity)
            .map(|remote| format!("git-remote:{remote}"));
        let checkout_is_temporary = is_temporary_checkout_path(&checkout.root);
        let git_basis = remote_basis.or_else(|| {
            git.common_git_dir
                .as_deref()
                .filter(|git_dir| !checkout_is_temporary || !is_temporary_checkout_path(git_dir))
                .map(|git_dir| format!("git-common-dir:{}", path_identity(git_dir)))
                .or_else(|| {
                    git.git_dir
                        .as_deref()
                        .filter(|git_dir| {
                            !checkout_is_temporary || !is_temporary_checkout_path(git_dir)
                        })
                        .map(|git_dir| format!("git-dir:{}", path_identity(git_dir)))
                })
        });
        let persistence = if git_basis.is_some() {
            RepoPersistence::Git
        } else {
            RepoPersistence::EphemeralPath
        };
        let repo_basis = git_basis
            .unwrap_or_else(|| format!("ephemeral-path:{}", path_identity(&checkout.root)));
        let repo_id = RepoId(stable_id("repo", &repo_basis));

        Self {
            repo_id,
            display_name: checkout.display_name.clone(),
            checkout_root: checkout.root.clone(),
            git_toplevel: git.toplevel.clone(),
            git_dir: git.git_dir.clone(),
            git_common_dir: git.common_git_dir.clone(),
            remote_url: git.remote_url.clone(),
            identity_basis: repo_basis,
            persistence,
        }
    }
}

/// Return whether a checkout path is owned by an operating-system temporary root.
pub fn is_temporary_checkout_path(path: &Path) -> bool {
    let path = path_identity(path);
    path == "/tmp"
        || path.starts_with("/tmp/")
        || path == "/private/tmp"
        || path.starts_with("/private/tmp/")
        || path == "/var/folders"
        || path.starts_with("/var/folders/")
        || path == "/private/var/folders"
        || path.starts_with("/private/var/folders/")
}

#[cfg(test)]
#[path = "../../tests/unit/state_core_temporary_checkout.rs"]
mod temporary_checkout_tests;

/// Workspace identity and the facts used to derive it.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceIdentity {
    pub workspace_id: WorkspaceId,
    pub display_name: String,
    pub root: PathBuf,
    pub git_dir: Option<PathBuf>,
    pub identity_basis: String,
    #[serde(default)]
    pub lifecycle: WorkspaceLifecycle,
}

/// Lifetime class for one checkout/worktree beneath a durable project.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum WorkspaceLifecycle {
    /// A checkout whose path is expected to remain available.
    #[default]
    Durable,
    /// A checkout under an operating-system temporary root.
    Temporary,
}

impl WorkspaceLifecycle {
    /// Return whether path-bound cache should follow temporary-workspace retention rules.
    pub const fn is_temporary(self) -> bool {
        matches!(self, Self::Temporary)
    }
}

impl WorkspaceIdentity {
    pub(super) fn from_checkout(
        git: &GitIdentity,
        checkout: &CheckoutIdentity,
        repo_id: &RepoId,
    ) -> Self {
        let workspace_basis = format!(
            "repo:{}|checkout:{}|git-dir:{}",
            repo_id.as_str(),
            path_identity(&checkout.root),
            git.git_dir
                .as_deref()
                .map(path_identity)
                .unwrap_or_else(|| "none".to_string())
        );
        let workspace_id = WorkspaceId(stable_id("workspace", &workspace_basis));

        Self {
            workspace_id,
            display_name: checkout.display_name.clone(),
            root: checkout.root.clone(),
            git_dir: git.git_dir.clone(),
            identity_basis: workspace_basis,
            lifecycle: if is_temporary_checkout_path(&checkout.root) {
                WorkspaceLifecycle::Temporary
            } else {
                WorkspaceLifecycle::Durable
            },
        }
    }
}

pub(super) fn stable_id(prefix: &str, value: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(value.as_bytes());
    let digest = hasher.finalize();
    format!("{prefix}-{:x}", digest)[..(prefix.len() + 1 + 16)].to_string()
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct CheckoutIdentity {
    root: PathBuf,
    display_name: String,
}

impl CheckoutIdentity {
    pub(super) fn new(cwd: &Path, git: &GitIdentity) -> Self {
        let root = git.toplevel.clone().unwrap_or_else(|| cwd.to_path_buf());
        let root = canonicalize_if_possible(&root);
        let display_name = root
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("workspace")
            .to_string();

        Self { root, display_name }
    }

    pub(super) fn root(&self) -> &Path {
        &self.root
    }
}
