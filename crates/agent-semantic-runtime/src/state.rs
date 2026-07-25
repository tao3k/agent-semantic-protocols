//! Materializes config-owned `ASP` project layout into runtime state directories.

use std::fs;
use std::path::{Path, PathBuf};

use agent_semantic_config::{ProjectRuntimeLayout, project_cache_root, project_runtime_layout};

/// Read-only ASP state paths derived from the config-owned project layout.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectStatePaths {
    pub layout: ProjectRuntimeLayout,
    pub repo_id: crate::state_core::RepoId,
    pub workspace_id: crate::state_core::WorkspaceId,
    pub identity_basis: String,
    pub persistence: crate::state_core::RepoPersistence,
    pub protocol_home: PathBuf,
    pub hook_cache_dir: PathBuf,
    pub hook_state_dir: PathBuf,
    pub activation_path: PathBuf,
    pub client_cache_dir: PathBuf,
    pub client_cache_manifest_path: PathBuf,
    pub project_client_db_dir: PathBuf,
    pub project_client_db_path: PathBuf,
    pub artifacts_dir: PathBuf,
    pub runtime_home: PathBuf,
    pub runtime_bin_dir: PathBuf,
    pub provider_bin_dir: PathBuf,
    pub provider_lock_dir: PathBuf,
}

/// Materialized project runtime state derived from the config-owned layout.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectRuntimeState {
    pub layout: ProjectRuntimeLayout,
    pub protocol_home: PathBuf,
    pub hook_cache_dir: PathBuf,
    pub hook_state_dir: PathBuf,
    pub activation_path: PathBuf,
    pub client_cache_dir: PathBuf,
    pub artifacts_dir: PathBuf,
    pub runtime_home: PathBuf,
    pub runtime_bin_dir: PathBuf,
    pub provider_bin_dir: PathBuf,
    pub provider_lock_dir: PathBuf,
}

/// Resolve the ASP runtime state paths for a project without creating files.
pub fn project_state_paths(project_root: impl AsRef<Path>) -> Result<ProjectStatePaths, String> {
    let layout = project_runtime_layout(project_root);
    let resolved = crate::state_core::ResolvedState::resolve(&layout.requested_root)?;
    Ok(project_state_paths_from_resolved(layout, resolved))
}

/// Resolve ASP runtime paths against an explicit State Home without creating files.
///
/// This boundary is intended for callers that already own State Home resolution
/// and for tests that must not mutate the process environment.
pub fn project_state_paths_with_state_home(
    project_root: impl AsRef<Path>,
    state_home: impl AsRef<Path>,
) -> Result<ProjectStatePaths, String> {
    let layout = project_runtime_layout(project_root);
    let resolved = crate::state_core::ResolvedState::resolve_with_state_home(
        &layout.requested_root,
        state_home,
    )?;
    Ok(project_state_paths_from_resolved(layout, resolved))
}

fn project_state_paths_from_resolved(
    layout: ProjectRuntimeLayout,
    resolved: crate::state_core::ResolvedState,
) -> ProjectStatePaths {
    let protocol_home = resolved.state_home.clone();
    let hook_dir = resolved.paths.hooks_dir.clone();
    let hook_cache_dir = hook_dir.join("cache");
    let hook_state_dir = hook_dir.join("state");
    let activation_path = hook_state_dir.join("activation.json");
    let client_cache_dir = resolved.paths.client_dir.clone();
    let project_client_db_dir = resolved.paths.project_client_dir.clone();
    let project_client_db_path = resolved.paths.project_client_db_path.clone();
    let artifacts_dir = resolved.paths.artifacts_dir.clone();
    let runtime_home = protocol_home.join("runtime");
    let runtime_bin_dir = runtime_home.join("bin");
    let provider_lock_dir = runtime_home.join("provider-locks");

    ProjectStatePaths {
        layout,
        repo_id: resolved.repo.repo_id.clone(),
        workspace_id: resolved.workspace.workspace_id.clone(),
        identity_basis: resolved.repo.identity_basis.clone(),
        persistence: resolved.repo.persistence,
        protocol_home,
        hook_cache_dir,
        hook_state_dir,
        activation_path,
        client_cache_dir,
        client_cache_manifest_path: resolved.paths.client_cache_manifest_path.clone(),
        project_client_db_dir,
        project_client_db_path,
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
    let layout = project_runtime_layout(project_root);
    let resolved = crate::state_core::ResolvedState::resolve(&layout.requested_root)?;
    resolved.ensure_minimal_layout()?;
    let paths = project_state_paths_from_resolved(layout, resolved);
    materialize_project_runtime_state(paths)
}

/// Resolve and create project runtime state beneath an explicit State Home.
pub fn project_runtime_state_with_state_home(
    project_root: impl AsRef<Path>,
    state_home: impl AsRef<Path>,
) -> Result<ProjectRuntimeState, String> {
    let layout = project_runtime_layout(project_root);
    let resolved = crate::state_core::ResolvedState::resolve_with_state_home(
        &layout.requested_root,
        state_home,
    )?;
    resolved.ensure_minimal_layout()?;
    let paths = project_state_paths_from_resolved(layout, resolved);
    materialize_project_runtime_state(paths)
}

fn materialize_project_runtime_state(
    paths: ProjectStatePaths,
) -> Result<ProjectRuntimeState, String> {
    let protocol_home = ensure_dir(paths.protocol_home)?;
    let hook_cache_dir = ensure_dir(paths.hook_cache_dir)?;
    let hook_state_dir = ensure_dir(paths.hook_state_dir)?;
    let client_cache_dir = ensure_dir(paths.client_cache_dir)?;
    let artifacts_dir = ensure_dir(paths.artifacts_dir)?;
    let runtime_home = ensure_dir(paths.runtime_home)?;
    let runtime_bin_dir = ensure_dir(paths.runtime_bin_dir)?;
    let provider_lock_dir = ensure_dir(paths.provider_lock_dir)?;

    Ok(ProjectRuntimeState {
        layout: paths.layout,
        protocol_home,
        hook_cache_dir,
        hook_state_dir,
        activation_path: paths.activation_path,
        client_cache_dir,
        artifacts_dir,
        runtime_home,
        runtime_bin_dir: runtime_bin_dir.clone(),
        provider_bin_dir: runtime_bin_dir,
        provider_lock_dir,
    })
}

/// Resolve the project-local ASP protocol home.
pub fn project_protocol_home_path(project_root: impl AsRef<Path>) -> Result<PathBuf, String> {
    Ok(project_state_paths(project_root)?.protocol_home)
}

/// Resolve the managed hook activation path.
pub fn project_activation_path(project_root: impl AsRef<Path>) -> Result<PathBuf, String> {
    Ok(project_state_paths(project_root)?.activation_path)
}

/// Resolve the managed hook activation path if it already exists.
pub fn discover_project_activation_path(project_root: impl AsRef<Path>) -> Option<PathBuf> {
    project_activation_path(project_root)
        .ok()
        .filter(|path| path.exists())
}

/// Return whether a path names an activation artifact.
pub fn is_project_activation_path(path: impl AsRef<Path>) -> bool {
    path.as_ref().file_name().and_then(|name| name.to_str()) == Some("activation.json")
}

/// Return the project root represented by an activation path when it is embedded
/// in the global State Core workspace layout.
pub fn project_root_for_activation_path(path: impl AsRef<Path>) -> Option<PathBuf> {
    let path = path.as_ref();
    if !is_project_activation_path(path) {
        return None;
    }
    project_root_for_state_activation_path(path)
}

fn project_root_for_state_activation_path(path: &Path) -> Option<PathBuf> {
    let state_dir = path.parent()?;
    if state_dir.file_name().and_then(|name| name.to_str()) != Some("state") {
        return None;
    }
    let hooks_dir = state_dir.parent()?;
    if hooks_dir.file_name().and_then(|name| name.to_str()) != Some("hooks") {
        return None;
    }
    let workspace_dir = hooks_dir.parent()?;
    let workspace_manifest = workspace_dir.join("workspace.json");
    let manifest = std::fs::read_to_string(workspace_manifest).ok()?;
    let manifest = serde_json::from_str::<serde_json::Value>(&manifest).ok()?;
    let root = manifest.get("root")?.as_str()?;
    if root.is_empty() {
        return None;
    }
    Some(PathBuf::from(root))
}

/// Return the runtime bin directory below an already-resolved cache home.
#[must_use]
pub fn runtime_bin_dir_for_cache_home(cache_home: impl AsRef<Path>) -> PathBuf {
    cache_home
        .as_ref()
        .join("agent-semantic-protocol")
        .join("runtime")
        .join("bin")
}

/// Resolve the cache root used by a project.
pub fn project_cache_home(project_root: impl AsRef<Path>) -> Result<PathBuf, String> {
    project_cache_root(project_root)
}

/// Resolve cache home for provider execution with activation-root fallback.
pub fn project_cache_home_for_roots(
    activation_root: &Path,
    project_root: &Path,
) -> Result<PathBuf, String> {
    project_cache_home(project_root).or_else(|project_error| {
        project_cache_home(activation_root).map_err(|activation_error| {
            format!(
                "{project_error}; failed to resolve activation-root cache home for {}: {activation_error}",
                activation_root.display()
            )
        })
    })
}

/// Resolve and create the managed hook activation directory.
pub fn ensure_project_hook_cache_dir(project_root: impl AsRef<Path>) -> Result<PathBuf, String> {
    ensure_dir(project_state_paths(project_root)?.hook_cache_dir)
}

/// Resolve and create the managed hook event-state directory.
pub fn ensure_project_hook_state_dir(project_root: impl AsRef<Path>) -> Result<PathBuf, String> {
    ensure_dir(project_state_paths(project_root)?.hook_state_dir)
}

/// Resolve and create the client cache directory.
pub fn ensure_project_client_cache_dir(project_root: impl AsRef<Path>) -> Result<PathBuf, String> {
    let paths = project_state_paths(project_root)?;
    crate::state_core::ResolvedState::resolve(&paths.layout.requested_root)?
        .ensure_minimal_layout()?;
    ensure_dir(paths.client_cache_dir)
}

/// Resolve and create the artifacts directory.
pub fn ensure_project_artifacts_dir(project_root: impl AsRef<Path>) -> Result<PathBuf, String> {
    let paths = project_state_paths(project_root)?;
    crate::state_core::ResolvedState::resolve(&paths.layout.requested_root)?
        .ensure_minimal_layout()?;
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
