//! Resolve checkout identity into one canonical State Core instance.

use super::identity::{
    CheckoutIdentity, RepoId, RepoIdentity, ScopeId, WorkspaceId, WorkspaceIdentity,
};
use super::layout::{
    ASP_STATE_HOME_ENV, DEFAULT_SCOPE_ID, STATE_LAYOUT_VERSION, StatePaths, TURSO_BACKEND,
    canonicalize_parent, resolve_state_home_from,
};
use crate::git::{GitIdentity, RemoteUrl, canonicalize_if_possible};
use serde::{Deserialize, Serialize};
use std::{
    env,
    path::{Path, PathBuf},
};

/// Fully resolved State Core identity and paths.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResolvedState {
    pub state_home: PathBuf,
    pub repo: RepoIdentity,
    pub workspace: WorkspaceIdentity,
    pub scope_id: ScopeId,
    pub paths: StatePaths,
}

impl ResolvedState {
    /// Resolve State Core from the process environment.
    pub fn resolve(cwd: impl AsRef<Path>) -> Result<Self, String> {
        let cwd = canonicalize_if_possible(cwd.as_ref());
        let explicit_state_home = env::var_os(ASP_STATE_HOME_ENV);
        let state_home = resolve_state_home_from(explicit_state_home, env::var_os("HOME"))?;
        Self::resolve_with_state_home(cwd, state_home)
    }

    /// Resolve State Core with an explicit state root.
    pub fn resolve_with_state_home(
        cwd: impl AsRef<Path>,
        state_home: impl AsRef<Path>,
    ) -> Result<Self, String> {
        let cwd = canonicalize_if_possible(cwd.as_ref());
        let state_home = canonicalize_parent(state_home.as_ref().to_path_buf());
        let git = GitIdentity::discover(&cwd);
        let checkout = CheckoutIdentity::new(&cwd, &git);
        let repo = RepoIdentity::from_checkout(&git, &checkout);
        let workspace = WorkspaceIdentity::from_checkout(&git, &checkout, &repo.repo_id);
        let paths = StatePaths::new(&state_home, &repo.repo_id, &workspace.workspace_id);
        Ok(Self {
            state_home,
            repo,
            workspace,
            scope_id: ScopeId(DEFAULT_SCOPE_ID.to_string()),
            paths,
        })
    }

    /// Render a diagnostic DTO for `asp state locate`.
    pub fn locate_report(&self) -> StateLocateReport {
        StateLocateReport {
            state_layout_version: STATE_LAYOUT_VERSION.to_string(),
            state_home: self.state_home.clone(),
            repo_id: self.repo.repo_id.clone(),
            workspace_id: self.workspace.workspace_id.clone(),
            scope_id: self.scope_id.clone(),
            repo_display_name: self.repo.display_name.clone(),
            workspace_display_name: self.workspace.display_name.clone(),
            checkout_root: self.workspace.root.clone(),
            git_toplevel: self.repo.git_toplevel.clone(),
            git_dir: self.workspace.git_dir.clone(),
            remote_url: self.repo.remote_url.clone(),
            persistence: self.repo.persistence,
            db_path: self.paths.client_db_path.clone(),
            artifact_path: self.paths.artifacts_dir.clone(),
            manifest_path: self.paths.client_manifest_json.clone(),
            generation_manifest_path: self.paths.client_cache_manifest_path.clone(),
            backend: TURSO_BACKEND.to_string(),
        }
    }
}

/// JSON-compatible diagnostic report for `asp state locate`.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StateLocateReport {
    pub state_layout_version: String,
    pub state_home: PathBuf,
    pub repo_id: RepoId,
    pub workspace_id: WorkspaceId,
    pub scope_id: ScopeId,
    pub repo_display_name: String,
    pub workspace_display_name: String,
    pub checkout_root: PathBuf,
    pub git_toplevel: Option<PathBuf>,
    pub git_dir: Option<PathBuf>,
    pub remote_url: Option<RemoteUrl>,
    pub persistence: super::identity::RepoPersistence,
    pub db_path: PathBuf,
    pub artifact_path: PathBuf,
    pub manifest_path: PathBuf,
    pub generation_manifest_path: PathBuf,
    pub backend: String,
}

/// Resolve state identity and optionally create the minimal v2 layout.
pub fn locate_state(
    cwd: impl AsRef<Path>,
    ensure_layout: bool,
) -> Result<StateLocateReport, String> {
    let state = ResolvedState::resolve(cwd)?;
    if ensure_layout {
        state.ensure_minimal_layout()?;
    }
    Ok(state.locate_report())
}
