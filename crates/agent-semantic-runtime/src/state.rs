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
    pub hook_cache_dir: PathBuf,
    pub hook_state_dir: PathBuf,
    pub activation_path: PathBuf,
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
    let hook_dir = workspace.hook_root();
    let hook_cache_dir = hook_dir.join("cache");
    let hook_state_dir = hook_dir.join("state");
    let activation_path = hook_state_dir.join("activation.json");
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
        hook_cache_dir,
        hook_state_dir,
        activation_path,
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
    let hook_cache_dir = ensure_dir(paths.hook_cache_dir)?;
    let hook_state_dir = ensure_dir(paths.hook_state_dir)?;
    let client_cache_dir = ensure_dir(paths.client_cache_dir)?;
    let artifacts_dir = ensure_dir(paths.artifacts_dir)?;
    let runtime_home = ensure_dir(paths.runtime_home)?;
    let runtime_bin_dir = ensure_dir(paths.runtime_bin_dir)?;
    let provider_lock_dir = ensure_dir(paths.provider_lock_dir)?;

    Ok(ProjectRuntimeState {
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

/// Resolve the State Core home used by this project identity.
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
    let observations_dir = hooks_dir.parent()?;
    if observations_dir.file_name().and_then(|name| name.to_str()) != Some("observations") {
        return None;
    }
    let binding_path = observations_dir.join(agent_semantic_artifacts::WORKSPACE_BINDING_FILE);
    let binding = serde_json::from_slice::<agent_semantic_artifacts::ProjectBinding>(
        &std::fs::read(binding_path).ok()?,
    )
    .ok()?;
    binding.validate().ok()?;
    Some(binding.workspace.canonical_root)
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
