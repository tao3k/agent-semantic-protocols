// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Materializes config-owned `ASP` project layout into runtime state directories.

use std::fs;
use std::path::Path;
use std::path::PathBuf;

/// Read-only ASP workspace/runtime paths derived through Artifacts.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkspaceRuntimePaths {
    pub repo_id: crate::state_core::RepoId,
    pub workspace_id: crate::state_core::WorkspaceId,
    pub identity_basis: String,
    pub persistence: crate::state_core::RepoPersistence,
    pub protocol_home: PathBuf,
    pub client_cache_dir: PathBuf,
    pub client_cache_manifest_path: PathBuf,
    pub client_db_dir: PathBuf,
    pub client_db_path: PathBuf,
    pub artifacts_dir: PathBuf,
    pub runtime_home: PathBuf,
    pub runtime_bin_dir: PathBuf,
    pub provider_bin_dir: PathBuf,
    pub provider_lock_dir: PathBuf,
}

/// Materialized runtime state derived from State Core.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectRuntimeState {
    pub protocol_home: PathBuf,
    pub client_cache_dir: PathBuf,
    pub artifacts_dir: PathBuf,
    pub runtime_home: PathBuf,
    pub runtime_bin_dir: PathBuf,
    pub provider_bin_dir: PathBuf,
    pub provider_lock_dir: PathBuf,
}

pub fn provider_state_root(protocol_home: impl AsRef<Path>) -> PathBuf {
    agent_semantic_artifacts::RuntimeArtifactStateLayout::new(protocol_home).provider_staging()
}

pub fn provider_receipt_dir(protocol_home: impl AsRef<Path>) -> PathBuf {
    provider_state_root(protocol_home).join("receipts")
}

pub fn provider_package_dir(protocol_home: impl AsRef<Path>) -> PathBuf {
    provider_state_root(protocol_home).join("packages")
}

/// Resolve the ASP runtime state paths for a project without creating files.
pub fn project_state_paths(
    project_root: impl AsRef<Path>,
) -> Result<WorkspaceRuntimePaths, String> {
    let resolved = crate::state_core::ResolvedState::resolve(project_root)?;
    Ok(project_state_paths_from_resolved(resolved))
}

/// Resolve ASP runtime paths against an explicit State Home without creating files.
///
/// This boundary is intended for callers that already own State Home resolution
/// and for tests that must not mutate the process environment.
pub fn project_state_paths_with_state_home(
    project_root: impl AsRef<Path>,
    state_home: impl AsRef<Path>,
) -> Result<WorkspaceRuntimePaths, String> {
    let resolved =
        crate::state_core::ResolvedState::resolve_with_state_home(project_root, state_home)?;
    Ok(project_state_paths_from_resolved(resolved))
}

fn project_state_paths_from_resolved(
    resolved: crate::state_core::ResolvedState,
) -> WorkspaceRuntimePaths {
    let protocol_home = resolved.state_home.clone();
    let workspace = resolved
        .workspace_state_paths()
        .expect("resolved workspace identity always maps to canonical State Home paths");
    let client_cache_dir = workspace.root.clone();
    let client_db_dir = workspace.root.clone();
    let client_db_path = workspace.facts.clone();
    let artifacts_dir = workspace.artifacts.clone();
    let runtime_layout =
        agent_semantic_artifacts::StateHomeLayout::new(&protocol_home).runtime_state();
    let runtime_home = runtime_layout.root().to_path_buf();
    let runtime_bin_dir = runtime_layout.artifacts().active_slot();
    let provider_lock_dir = provider_receipt_dir(&protocol_home);

    WorkspaceRuntimePaths {
        repo_id: resolved.repo.repo_id.clone(),
        workspace_id: resolved.workspace.workspace_id.clone(),
        identity_basis: resolved.repo.identity_basis.clone(),
        persistence: resolved.repo.persistence,
        protocol_home,
        client_cache_dir,
        client_cache_manifest_path: workspace.cache_manifest_path(),
        client_db_dir,
        client_db_path,
        artifacts_dir,
        runtime_home,
        runtime_bin_dir: runtime_bin_dir.clone(),
        provider_bin_dir: runtime_bin_dir,
        provider_lock_dir,
    }
}

/// Resolve and create the ASP runtime state directories for a project.
pub fn project_runtime_state(
    project_root: impl AsRef<Path>,
) -> Result<ProjectRuntimeState, String> {
    let resolved = crate::state_core::ResolvedState::resolve(project_root)?;
    resolved.ensure_workspace_state_layout()?;
    let paths = project_state_paths_from_resolved(resolved);
    materialize_project_runtime_state(paths)
}

/// Resolve and create project runtime state beneath an explicit State Home.
pub fn project_runtime_state_with_state_home(
    project_root: impl AsRef<Path>,
    state_home: impl AsRef<Path>,
) -> Result<ProjectRuntimeState, String> {
    let resolved =
        crate::state_core::ResolvedState::resolve_with_state_home(project_root, state_home)?;
    resolved.ensure_workspace_state_layout()?;
    let paths = project_state_paths_from_resolved(resolved);
    materialize_project_runtime_state(paths)
}

/// Materialize a temporary Git checkout as a workspace beneath an explicit owner project.
pub fn temporary_workspace_runtime_state_with_owner_and_state_home(
    temporary_root: impl AsRef<Path>,
    owner_project_root: impl AsRef<Path>,
    state_home: impl AsRef<Path>,
) -> Result<ProjectRuntimeState, String> {
    let resolved =
        crate::state_core::ResolvedState::resolve_temporary_workspace_with_owner_and_state_home(
            temporary_root,
            owner_project_root,
            state_home,
        )?;
    resolved.ensure_workspace_state_layout()?;
    let paths = project_state_paths_from_resolved(resolved);
    materialize_project_runtime_state(paths)
}

fn materialize_project_runtime_state(
    paths: WorkspaceRuntimePaths,
) -> Result<ProjectRuntimeState, String> {
    let protocol_home = ensure_dir(paths.protocol_home)?;
    let client_cache_dir = ensure_dir(paths.client_cache_dir)?;
    let artifacts_dir = ensure_dir(paths.artifacts_dir)?;
    let runtime_home = ensure_dir(paths.runtime_home)?;
    let runtime_bin_dir = ensure_dir(paths.runtime_bin_dir)?;
    let provider_lock_dir = ensure_dir(paths.provider_lock_dir)?;

    Ok(ProjectRuntimeState {
        protocol_home,
        client_cache_dir,
        artifacts_dir,
        runtime_home,
        runtime_bin_dir: runtime_bin_dir.clone(),
        provider_bin_dir: runtime_bin_dir,
        provider_lock_dir,
    })
}

/// Resolve the State Core home used by this project identity.
pub fn project_protocol_home_path(project_root: impl AsRef<Path>) -> Result<PathBuf, String> {
    Ok(project_state_paths(project_root)?.protocol_home)
}

/// Resolve and create the client cache directory.
pub fn ensure_project_client_cache_dir(project_root: impl AsRef<Path>) -> Result<PathBuf, String> {
    let project_root = project_root.as_ref();
    let paths = project_state_paths(project_root)?;
    crate::state_core::ResolvedState::resolve(project_root)?.ensure_workspace_state_layout()?;
    ensure_dir(paths.client_cache_dir)
}

/// Resolve and create the artifacts directory.
pub fn ensure_project_artifacts_dir(project_root: impl AsRef<Path>) -> Result<PathBuf, String> {
    let project_root = project_root.as_ref();
    let paths = project_state_paths(project_root)?;
    crate::state_core::ResolvedState::resolve(project_root)?.ensure_workspace_state_layout()?;
    ensure_dir(paths.artifacts_dir)
}

/// Resolve and create the runtime command-shim directory.
pub fn ensure_project_runtime_home(project_root: impl AsRef<Path>) -> Result<PathBuf, String> {
    ensure_dir(project_state_paths(project_root)?.runtime_home)
}

/// Resolve and create the managed provider binary directory.
pub fn ensure_project_provider_bin_dir(project_root: impl AsRef<Path>) -> Result<PathBuf, String> {
    ensure_dir(project_state_paths(project_root)?.provider_bin_dir)
}

/// Resolve and create the managed provider release lock directory.
pub fn ensure_project_provider_lock_dir(project_root: impl AsRef<Path>) -> Result<PathBuf, String> {
    ensure_dir(project_state_paths(project_root)?.provider_lock_dir)
}

fn ensure_dir(path: PathBuf) -> Result<PathBuf, String> {
    fs::create_dir_all(&path)
        .map_err(|error| format!("failed to create {}: {error}", path.display()))?;
    Ok(path)
}

#[cfg(test)]
#[path = "../tests/unit/state.rs"]
mod state_tests;
