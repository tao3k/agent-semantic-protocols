//! Materializes config-owned `ASP` project layout into runtime state directories.

use std::fs;
use std::path::{Path, PathBuf};

use agent_semantic_config::{
    ProjectRuntimeLayout, project_artifacts_dir, project_client_cache_dir, project_hook_cache_dir,
    project_hook_state_dir, project_runtime_layout,
};

/// Materialized project runtime state derived from the config-owned layout.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectRuntimeState {
    pub layout: ProjectRuntimeLayout,
    pub hook_cache_dir: PathBuf,
    pub hook_state_dir: PathBuf,
    pub client_cache_dir: PathBuf,
    pub artifacts_dir: PathBuf,
    pub runtime_home: PathBuf,
    pub provider_bin_dir: PathBuf,
    pub provider_lock_dir: PathBuf,
}

/// Resolve and create the ASP runtime state directories for a project.
pub fn project_runtime_state(
    project_root: impl AsRef<Path>,
) -> Result<ProjectRuntimeState, String> {
    let project_root = project_root.as_ref();
    let layout = project_runtime_layout(project_root);
    let hook_cache_dir = ensure_layout_dir(&layout, "hook cache", layout.hook_cache_dir.as_ref())?;
    let hook_state_dir = ensure_layout_dir(&layout, "hook state", layout.hook_cache_dir.as_ref())?;
    let client_cache_dir =
        ensure_layout_dir(&layout, "client cache", layout.client_cache_dir.as_ref())?;
    let artifacts_dir = ensure_layout_dir(&layout, "artifacts", layout.artifacts_dir.as_ref())?;
    let runtime_home = ensure_layout_dir(&layout, "runtime home", layout.runtime_home.as_ref())?;
    let provider_bin_dir = ensure_dir(runtime_home.join("bin"))?;
    let provider_lock_dir = ensure_dir(runtime_home.join("providers"))?;

    Ok(ProjectRuntimeState {
        layout,
        hook_cache_dir,
        hook_state_dir,
        client_cache_dir,
        artifacts_dir,
        runtime_home,
        provider_bin_dir,
        provider_lock_dir,
    })
}

fn ensure_layout_dir(
    layout: &ProjectRuntimeLayout,
    label: &str,
    path: Option<&PathBuf>,
) -> Result<PathBuf, String> {
    let path = path.ok_or_else(|| {
        format!(
            "failed to locate ASP {label} for {}",
            layout.requested_root.display()
        )
    })?;
    ensure_dir(path.clone())
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
    Ok(ensure_project_runtime_home(project_root)?.join("bin")).and_then(ensure_dir)
}

/// Resolve and create the managed provider release lock directory.
pub fn ensure_project_provider_lock_dir(project_root: impl AsRef<Path>) -> Result<PathBuf, String> {
    Ok(ensure_project_runtime_home(project_root)?.join("providers")).and_then(ensure_dir)
}

fn ensure_dir(path: PathBuf) -> Result<PathBuf, String> {
    fs::create_dir_all(&path)
        .map_err(|error| format!("failed to create {}: {error}", path.display()))?;
    Ok(path)
}

#[cfg(test)]
#[path = "../tests/unit/state.rs"]
mod state_tests;
