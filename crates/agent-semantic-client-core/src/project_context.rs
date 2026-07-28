//! Project-facing entry point for ASP state and cache paths.

use std::path::{Path, PathBuf};

/// Resolved project identity and state-layout roots for client-owned storage.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectContext {
    cwd: PathBuf,
    git_toplevel: Option<PathBuf>,
    project_home: Option<PathBuf>,
    state_layout: StateLayout,
}

/// Single interface for cache and client state locations.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StateLayout {
    state_root: PathBuf,
    client_cache_dir: PathBuf,
    cache_manifest_path: PathBuf,
    artifacts_dir: PathBuf,
}

impl ProjectContext {
    pub fn resolve(cwd: impl AsRef<Path>) -> Result<Self, String> {
        let cwd = canonicalize_if_possible(cwd.as_ref());
        let resolved = crate::state_core::ResolvedState::resolve(&cwd)?;
        let git_toplevel = resolved.repo.git_toplevel.clone();
        let project_home = git_toplevel.clone();
        let state_layout = StateLayout::from_resolved_state(resolved)?;

        Ok(Self {
            cwd,
            git_toplevel,
            project_home,
            state_layout,
        })
    }

    pub fn cwd(&self) -> &Path {
        &self.cwd
    }

    pub fn git_toplevel(&self) -> Option<&Path> {
        self.git_toplevel.as_deref()
    }

    pub fn project_home(&self) -> Option<&Path> {
        self.project_home.as_deref()
    }

    pub fn state_layout(&self) -> &StateLayout {
        &self.state_layout
    }

    pub fn require_inside_workspace(&self, path: impl AsRef<Path>) -> Result<PathBuf, String> {
        let path = canonicalize_if_possible(path.as_ref());
        let Some(workspace_root) = self.git_toplevel() else {
            return Err(format!(
                "workspace boundary is unavailable for {}",
                self.cwd.display()
            ));
        };
        if path.starts_with(workspace_root) {
            Ok(path)
        } else {
            Err(format!(
                "path {} is outside workspace {}",
                path.display(),
                workspace_root.display()
            ))
        }
    }
}

impl StateLayout {
    pub fn resolve(project_root: impl AsRef<Path>) -> Result<Self, String> {
        Self::from_resolved_state(crate::state_core::ResolvedState::resolve(project_root)?)
    }

    fn from_resolved_state(resolved: crate::state_core::ResolvedState) -> Result<Self, String> {
        resolved.ensure_minimal_layout()?;
        let state_root = resolved.state_home.clone();
        let client_cache_dir = resolved.paths.client_dir.clone();
        let artifacts_dir = resolved.paths.artifacts_dir.clone();
        let cache_manifest_path = resolved.paths.client_cache_manifest_path.clone();

        Ok(Self {
            state_root,
            client_cache_dir,
            cache_manifest_path,
            artifacts_dir,
        })
    }

    pub fn state_root(&self) -> &Path {
        &self.state_root
    }

    pub fn client_cache_dir(&self) -> &Path {
        &self.client_cache_dir
    }

    pub fn cache_manifest_path(&self) -> &Path {
        &self.cache_manifest_path
    }

    pub fn artifacts_dir(&self) -> &Path {
        &self.artifacts_dir
    }
}

fn canonicalize_if_possible(path: &Path) -> PathBuf {
    path.canonicalize().unwrap_or_else(|_| path.to_path_buf())
}
