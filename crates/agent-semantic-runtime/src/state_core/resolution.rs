// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Resolve checkout identity into one canonical State Core instance.

use super::identity::CheckoutIdentity;
use super::identity::RepoId;
use super::identity::RepoIdentity;
use super::identity::ScopeId;
use super::identity::WorkspaceId;
use super::identity::WorkspaceIdentity;
use super::layout::ASP_STATE_HOME_ENV;
use super::layout::DEFAULT_SCOPE_ID;
use super::layout::STATE_LAYOUT_VERSION;
use super::layout::TURSO_BACKEND;
use super::layout::canonicalize_parent;
use super::layout::resolve_state_home_from;
use crate::git::GitIdentity;
use crate::git::RemoteUrl;
use crate::git::canonicalize_if_possible;
use serde::Deserialize;
use serde::Serialize;
use std::env;
use std::path::Path;
use std::path::PathBuf;

/// Fully resolved State Core identity and paths.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResolvedState {
    pub state_home: PathBuf,
    pub repo: RepoIdentity,
    pub workspace: WorkspaceIdentity,
    pub scope_id: ScopeId,
}

impl ResolvedState {
    /// Resolve the content-addressed binding used by all normal State Home writers.
    pub fn project_binding(&self) -> Result<agent_semantic_artifacts::ProjectBinding, String> {
        agent_semantic_artifacts::ProjectBinding::resolve_with_private_git_dir(
            None,
            self.repo.identity_basis.clone(),
            &self.workspace.root,
            self.workspace.git_dir.as_deref(),
        )
    }

    /// Resolve canonical workspace paths without materializing them.
    pub fn workspace_state_paths(
        &self,
    ) -> Result<agent_semantic_artifacts::WorkspaceStatePaths, String> {
        let binding = self.project_binding()?;
        agent_semantic_artifacts::StateHomeLayout::new(&self.state_home)
            .workspace(&binding.workspace)
    }

    /// Materialize the canonical content-addressed workspace envelope.
    pub fn ensure_workspace_state_layout(
        &self,
    ) -> Result<agent_semantic_artifacts::WorkspaceStatePaths, String> {
        if !self.repo.persistence.is_durable() {
            return Err(format!(
                "refusing to materialize ephemeral or standalone temporary checkout: {}",
                self.repo.checkout_root.display()
            ));
        }
        let binding = self.project_binding()?;
        agent_semantic_artifacts::StateHomeLayout::new(&self.state_home)
            .materialize_workspace(&binding)
    }

    pub async fn ensure_workspace_state_layout_async(
        &self,
    ) -> Result<agent_semantic_artifacts::WorkspaceStatePaths, String> {
        let resolved = self.clone();
        tokio::task::spawn_blocking(move || resolved.ensure_workspace_state_layout())
            .await
            .map_err(|error| format!("canonical workspace materialization task failed: {error}"))?
    }

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
        Ok(Self {
            state_home,
            repo,
            workspace,
            scope_id: ScopeId(DEFAULT_SCOPE_ID.to_string()),
        })
    }

    /// Resolve a temporary checkout as a workspace owned by a durable project checkout.
    pub fn resolve_temporary_workspace_with_owner_and_state_home(
        temporary_cwd: impl AsRef<Path>,
        owner_cwd: impl AsRef<Path>,
        state_home: impl AsRef<Path>,
    ) -> Result<Self, String> {
        let temporary_cwd = canonicalize_if_possible(temporary_cwd.as_ref());
        let owner_cwd = canonicalize_if_possible(owner_cwd.as_ref());
        let state_home = canonicalize_parent(state_home.as_ref().to_path_buf());
        let temporary_git = GitIdentity::discover(&temporary_cwd);
        let temporary_checkout = CheckoutIdentity::new(&temporary_cwd, &temporary_git);
        if !super::is_temporary_checkout_path(temporary_checkout.root()) {
            return Err(format!(
                "owner-bound temporary workspace must be under an operating-system temporary root: {}",
                temporary_checkout.root().display()
            ));
        }
        if temporary_git.toplevel.is_none() {
            return Err(format!(
                "owner-bound temporary workspace is not a Gix repository: {}",
                temporary_checkout.root().display()
            ));
        }

        let owner_git = GitIdentity::discover(&owner_cwd);
        let owner_checkout = CheckoutIdentity::new(&owner_cwd, &owner_git);
        let repo = RepoIdentity::from_checkout(&owner_git, &owner_checkout);
        if !repo.persistence.is_durable() {
            return Err(format!(
                "temporary workspace owner is not a durable project: {}",
                owner_checkout.root().display()
            ));
        }
        let discovered_temporary_repo =
            RepoIdentity::from_checkout(&temporary_git, &temporary_checkout);
        if discovered_temporary_repo.persistence.is_durable()
            && discovered_temporary_repo.repo_id != repo.repo_id
        {
            return Err(format!(
                "Gix resolved temporary checkout {} to project {}, not owner project {}",
                temporary_checkout.root().display(),
                discovered_temporary_repo.repo_id.as_str(),
                repo.repo_id.as_str()
            ));
        }
        let workspace =
            WorkspaceIdentity::from_checkout(&temporary_git, &temporary_checkout, &repo.repo_id);
        Ok(Self {
            state_home,
            repo,
            workspace,
            scope_id: ScopeId(DEFAULT_SCOPE_ID.to_string()),
        })
    }

    /// Render a diagnostic DTO for `asp state locate`.
    pub fn locate_report(&self) -> StateLocateReport {
        let workspace = self
            .workspace_state_paths()
            .expect("resolved State Home workspace identity is canonical");
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
            db_path: workspace.facts.clone(),
            artifact_path: workspace.artifacts.clone(),
            manifest_path: workspace.db_manifest_path(),
            generation_manifest_path: workspace.cache_manifest_path(),
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
        state.ensure_workspace_state_layout()?;
    }
    Ok(state.locate_report())
}
