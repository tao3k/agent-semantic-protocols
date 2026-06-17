//! Materializes config-owned `ASP` project layout into runtime state directories.

use std::fs;
use std::path::{Path, PathBuf};

use agent_semantic_config::{
    ProjectRuntimeLayout, project_activation_path as config_project_activation_path,
    project_artifacts_dir, project_cache_root, project_client_cache_dir, project_hook_cache_dir,
    project_hook_state_dir, project_protocol_home, project_provider_lock_dir,
    project_runtime_bin_dir, project_runtime_layout,
};

/// Read-only ASP state paths derived from the config-owned project layout.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectStatePaths {
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
    let protocol_home =
        require_layout_path(&layout, "protocol home", layout.protocol_home.as_ref())?;
    let hook_cache_dir =
        require_layout_path(&layout, "hook cache", layout.hook_cache_dir.as_ref())?;
    let hook_state_dir =
        require_layout_path(&layout, "hook state", layout.hook_state_dir.as_ref())?;
    let activation_path =
        require_layout_path(&layout, "hook activation", layout.activation_path.as_ref())?;
    let client_cache_dir =
        require_layout_path(&layout, "client cache", layout.client_cache_dir.as_ref())?;
    let artifacts_dir = require_layout_path(&layout, "artifacts", layout.artifacts_dir.as_ref())?;
    let runtime_home = require_layout_path(&layout, "runtime home", layout.runtime_home.as_ref())?;
    let runtime_bin_dir =
        require_layout_path(&layout, "runtime bin", layout.runtime_bin_dir.as_ref())?;
    let provider_lock_dir =
        require_layout_path(&layout, "provider locks", layout.provider_lock_dir.as_ref())?;

    Ok(ProjectStatePaths {
        layout,
        protocol_home,
        hook_cache_dir,
        hook_state_dir,
        activation_path,
        client_cache_dir,
        artifacts_dir,
        runtime_home,
        runtime_bin_dir: runtime_bin_dir.clone(),
        provider_bin_dir: runtime_bin_dir,
        provider_lock_dir,
    })
}

/// Resolve and create the ASP runtime state directories for a project.
pub fn project_runtime_state(
    project_root: impl AsRef<Path>,
) -> Result<ProjectRuntimeState, String> {
    let paths = project_state_paths(project_root)?;
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

fn require_layout_path(
    layout: &ProjectRuntimeLayout,
    label: &str,
    path: Option<&PathBuf>,
) -> Result<PathBuf, String> {
    path.cloned().ok_or_else(|| {
        format!(
            "failed to locate ASP {label} for {}",
            layout.requested_root.display()
        )
    })
}

/// Resolve the project-local ASP protocol home.
pub fn project_protocol_home_path(project_root: impl AsRef<Path>) -> Result<PathBuf, String> {
    project_protocol_home(project_root)
}

/// Resolve the managed hook activation path.
pub fn project_activation_path(project_root: impl AsRef<Path>) -> Result<PathBuf, String> {
    config_project_activation_path(project_root)
}

/// Return the conventional activation path below a candidate project root.
#[must_use]
pub fn project_local_activation_path(project_root: impl AsRef<Path>) -> PathBuf {
    project_root
        .as_ref()
        .join(".cache")
        .join("agent-semantic-protocol")
        .join("hooks")
        .join("activation.json")
}

/// Return the conventional client cache manifest path below a candidate root.
#[must_use]
pub fn project_local_client_cache_manifest_path(project_root: impl AsRef<Path>) -> PathBuf {
    project_root
        .as_ref()
        .join(".cache")
        .join("agent-semantic-protocol")
        .join("client")
        .join("cache-manifest.json")
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

/// Search ancestors for a managed hook activation path.
pub fn discover_project_activation_path(start: &Path) -> Option<PathBuf> {
    start
        .ancestors()
        .map(project_local_activation_path)
        .find(|path| path.is_file())
}

/// Return whether a path is the project-local managed activation path shape.
#[must_use]
pub fn is_project_activation_path(path: &Path) -> bool {
    project_root_for_activation_path(path).is_some()
}

/// Recover the project root from a project-local managed activation path.
#[must_use]
pub fn project_root_for_activation_path(activation_path: &Path) -> Option<PathBuf> {
    let file_name = activation_path.file_name()?.to_str()?;
    if file_name != "activation.json" {
        return None;
    }
    let hooks_dir = activation_path.parent()?;
    if hooks_dir.file_name()?.to_str()? != "hooks" {
        return None;
    }
    let protocol_home = hooks_dir.parent()?;
    if protocol_home.file_name()?.to_str()? != "agent-semantic-protocol" {
        return None;
    }
    let cache_home = protocol_home.parent()?;
    if cache_home.file_name()?.to_str()? != ".cache" {
        return None;
    }
    cache_home
        .parent()
        .map(|path| path.canonicalize().unwrap_or_else(|_| path.to_path_buf()))
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
    ensure_dir(project_hook_cache_dir(project_root)?)
}

/// Resolve and create the managed hook event-state directory.
pub fn ensure_project_hook_state_dir(project_root: impl AsRef<Path>) -> Result<PathBuf, String> {
    ensure_dir(project_hook_state_dir(project_root)?)
}

/// Resolve and create the client cache directory.
pub fn ensure_project_client_cache_dir(project_root: impl AsRef<Path>) -> Result<PathBuf, String> {
    ensure_dir(project_client_cache_dir(project_root)?)
}

/// Resolve and create the artifacts directory.
pub fn ensure_project_artifacts_dir(project_root: impl AsRef<Path>) -> Result<PathBuf, String> {
    ensure_dir(project_artifacts_dir(project_root)?)
}

/// Resolve and create the runtime command-shim directory.
pub fn ensure_project_runtime_home(project_root: impl AsRef<Path>) -> Result<PathBuf, String> {
    let layout = project_runtime_layout(project_root);
    let runtime_home = layout.runtime_home.ok_or_else(|| {
        format!(
            "failed to locate ASP runtime home for {}",
            layout.requested_root.display()
        )
    })?;
    ensure_dir(runtime_home)
}

/// Resolve and create the managed provider binary directory.
pub fn ensure_project_provider_bin_dir(project_root: impl AsRef<Path>) -> Result<PathBuf, String> {
    ensure_dir(project_runtime_bin_dir(project_root)?)
}

/// Resolve and create the managed provider release lock directory.
pub fn ensure_project_provider_lock_dir(project_root: impl AsRef<Path>) -> Result<PathBuf, String> {
    ensure_dir(project_provider_lock_dir(project_root)?)
}

fn ensure_dir(path: PathBuf) -> Result<PathBuf, String> {
    fs::create_dir_all(&path)
        .map_err(|error| format!("failed to create {}: {error}", path.display()))?;
    Ok(path)
}

#[cfg(test)]
#[path = "../tests/unit/state.rs"]
mod state_tests;
