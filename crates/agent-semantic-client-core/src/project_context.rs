use std::path::{Path, PathBuf};

use agent_semantic_config::{ProjectEnvStatus, ProjectRuntimeLayout, project_runtime_layout};

use crate::AGENT_SEMANTIC_CLIENT_CACHE_MANIFEST_FILE;

/// Resolved project identity and state-layout roots for client-owned storage.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectContext {
    cwd: PathBuf,
    git_toplevel: Option<PathBuf>,
    project_home: Option<PathBuf>,
    project_env: ProjectEnvStatus,
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
        let runtime_layout = project_runtime_layout(&cwd);
        let git_toplevel = runtime_layout.git_toplevel.clone();
        let project_home = runtime_layout.project_home.clone();
        let project_env = runtime_layout.project_env.clone();
        let state_layout = StateLayout::from_runtime_layout(&runtime_layout)?;

        Ok(Self {
            cwd,
            git_toplevel,
            project_home,
            project_env,
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

    pub fn project_env(&self) -> &ProjectEnvStatus {
        &self.project_env
    }

    pub fn prj_env_vars_available(&self) -> bool {
        matches!(
            self.project_env,
            ProjectEnvStatus::DirenvAtGitToplevel { .. }
        )
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
        Self::from_runtime_layout(&project_runtime_layout(project_root))
    }

    fn from_runtime_layout(layout: &ProjectRuntimeLayout) -> Result<Self, String> {
        let state_root = layout.protocol_home.clone().ok_or_else(|| {
            format!(
                "failed to locate ASP state root for {}",
                layout.requested_root.display()
            )
        })?;
        let client_cache_dir = layout.client_cache_dir.clone().ok_or_else(|| {
            format!(
                "failed to locate ASP client cache dir for {}",
                layout.requested_root.display()
            )
        })?;
        let artifacts_dir = layout.artifacts_dir.clone().ok_or_else(|| {
            format!(
                "failed to locate ASP artifact dir for {}",
                layout.requested_root.display()
            )
        })?;
        let cache_manifest_path = client_cache_dir.join(AGENT_SEMANTIC_CLIENT_CACHE_MANIFEST_FILE);

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
