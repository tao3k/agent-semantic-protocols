//! Project identity and filesystem layout for local ASP runtime state.

use std::env;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Legacy typo-like environment variable reported by healthcheck but not used.
pub const PRJ_HOME_CACHE_ENV: &str = "PRJ_HOME_CACHE";
/// Canonical environment variable for project-local ASP cache state.
pub const PRJ_CACHE_HOME_ENV: &str = "PRJ_CACHE_HOME";

const SEMANTIC_AGENT_PROTOCOL_DIR: &str = "agent-semantic-protocol";
const SEMANTIC_AGENT_PROTOCOL_HOOK_DIR: &str = "agent-semantic-protocol/hooks";
const SEMANTIC_AGENT_PROTOCOL_CLIENT_DIR: &str = "agent-semantic-protocol/client";
const SEMANTIC_AGENT_PROTOCOL_ARTIFACTS_DIR: &str = "agent-semantic-protocol/artifacts";
const SEMANTIC_AGENT_PROTOCOL_RUNTIME_DIR: &str = "agent-semantic-protocol/runtime";

/// Source used to resolve the project-local cache home.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProjectCacheSource {
    /// Cache home came from `PRJ_CACHE_HOME`.
    PrjCacheHome,
    /// Cache home came from `<git-toplevel>/.cache`.
    GitToplevel,
}

impl ProjectCacheSource {
    /// Return the compact healthcheck label for this cache source.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::PrjCacheHome => "prj-cache-home",
            Self::GitToplevel => "git-toplevel",
        }
    }
}

/// Read-only project runtime layout derived from git and ASP cache conventions.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectRuntimeLayout {
    /// Input root used by the caller before git normalization.
    pub requested_root: PathBuf,
    /// Git toplevel for this project identity.
    pub git_toplevel: Option<PathBuf>,
    /// Cache home used for state storage.
    pub cache_home: Option<PathBuf>,
    /// Source that selected `cache_home`.
    pub cache_source: Option<ProjectCacheSource>,
    /// Legacy typo-like cache environment value, reported but ignored.
    pub prj_home_cache: Option<PathBuf>,
    /// Canonical project cache override value.
    pub prj_cache_home: Option<PathBuf>,
    /// Protocol root below `cache_home`.
    pub protocol_home: Option<PathBuf>,
    /// Hook activation and event-state directory.
    pub hook_cache_dir: Option<PathBuf>,
    /// Hook activation path.
    pub activation_path: Option<PathBuf>,
    /// Client manifest and SQLite directory.
    pub client_cache_dir: Option<PathBuf>,
    /// Provider/client artifact directory.
    pub artifacts_dir: Option<PathBuf>,
    /// Runtime profile and command-shim directory.
    pub runtime_home: Option<PathBuf>,
    /// Runtime profile path.
    pub runtime_profiles_path: Option<PathBuf>,
    /// Agent skill/config directory under git toplevel.
    pub agents_dir: Option<PathBuf>,
    /// Installed ASP skill path under `.agents`.
    pub agent_skill_path: Option<PathBuf>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct ProjectRuntimeEnv {
    prj_home_cache: Option<PathBuf>,
    prj_cache_home: Option<PathBuf>,
}

impl ProjectRuntimeEnv {
    fn from_process() -> Self {
        Self {
            prj_home_cache: env_path(PRJ_HOME_CACHE_ENV),
            prj_cache_home: env_path(PRJ_CACHE_HOME_ENV),
        }
    }
}

/// Resolve all project-local runtime paths without creating or refreshing state.
pub fn project_runtime_layout(project_root: impl AsRef<Path>) -> ProjectRuntimeLayout {
    let project_root = project_root.as_ref();
    project_runtime_layout_with_env(project_root, ProjectRuntimeEnv::from_process())
}

fn project_runtime_layout_with_env(
    project_root: &Path,
    runtime_env: ProjectRuntimeEnv,
) -> ProjectRuntimeLayout {
    let requested_root = project_root.to_path_buf();
    let git_toplevel = local_git_toplevel(project_root)
        .or_else(|| command_git_toplevel(project_root))
        .map(canonicalize_if_possible);
    let prj_home_cache = runtime_env.prj_home_cache;
    let prj_cache_home = runtime_env.prj_cache_home;

    let (cache_home, cache_source) = if let Some(cache_root) = prj_cache_home.clone() {
        (Some(cache_root), Some(ProjectCacheSource::PrjCacheHome))
    } else if let Some(git_toplevel) = git_toplevel.as_ref() {
        (
            Some(git_toplevel.join(".cache")),
            Some(ProjectCacheSource::GitToplevel),
        )
    } else {
        (None, None)
    };

    let protocol_home = cache_home
        .as_ref()
        .map(|cache_home| cache_home.join(SEMANTIC_AGENT_PROTOCOL_DIR));
    let hook_cache_dir = cache_home
        .as_ref()
        .map(|cache_home| cache_home.join(SEMANTIC_AGENT_PROTOCOL_HOOK_DIR));
    let activation_path = hook_cache_dir
        .as_ref()
        .map(|hook_cache_dir| hook_cache_dir.join("activation.json"));
    let client_cache_dir = cache_home
        .as_ref()
        .map(|cache_home| cache_home.join(SEMANTIC_AGENT_PROTOCOL_CLIENT_DIR));
    let artifacts_dir = cache_home
        .as_ref()
        .map(|cache_home| cache_home.join(SEMANTIC_AGENT_PROTOCOL_ARTIFACTS_DIR));
    let runtime_home = cache_home
        .as_ref()
        .map(|cache_home| cache_home.join(SEMANTIC_AGENT_PROTOCOL_RUNTIME_DIR));
    let runtime_profiles_path = runtime_home
        .as_ref()
        .map(|runtime_home| runtime_home.join("profiles.json"));
    let agents_dir = git_toplevel
        .as_ref()
        .map(|git_toplevel| git_toplevel.join(".agents"));
    let agent_skill_path = agents_dir
        .as_ref()
        .map(|agents_dir| agents_dir.join("skills/agent-semantic-protocols/SKILL.md"));

    ProjectRuntimeLayout {
        requested_root,
        git_toplevel,
        cache_home,
        cache_source,
        prj_home_cache,
        prj_cache_home,
        protocol_home,
        hook_cache_dir,
        activation_path,
        client_cache_dir,
        artifacts_dir,
        runtime_home,
        runtime_profiles_path,
        agents_dir,
        agent_skill_path,
    }
}

/// Resolve the project cache directory for hook activation and profiles.
pub fn project_hook_cache_dir(project_root: impl AsRef<Path>) -> Result<PathBuf, String> {
    Ok(project_cache_root(project_root)?.join(SEMANTIC_AGENT_PROTOCOL_HOOK_DIR))
}

/// Resolve the project cache directory used for hook event state.
pub fn project_hook_state_dir(project_root: impl AsRef<Path>) -> Result<PathBuf, String> {
    project_hook_cache_dir(project_root)
}

/// Return the agent semantic client cache directory for an activated project.
pub fn project_client_cache_dir(project_root: impl AsRef<Path>) -> Result<PathBuf, String> {
    Ok(project_cache_root(project_root)?.join(SEMANTIC_AGENT_PROTOCOL_CLIENT_DIR))
}

/// Return the agent semantic artifacts directory for an activated project.
pub fn project_artifacts_dir(project_root: impl AsRef<Path>) -> Result<PathBuf, String> {
    Ok(project_cache_root(project_root)?.join(SEMANTIC_AGENT_PROTOCOL_ARTIFACTS_DIR))
}

/// Resolve the default runtime profiles path for a project.
pub fn default_runtime_profiles_path(project_root: impl AsRef<Path>) -> Result<PathBuf, String> {
    Ok(project_cache_root(project_root)?
        .join(SEMANTIC_AGENT_PROTOCOL_RUNTIME_DIR)
        .join("profiles.json"))
}

/// Resolve the runtime profiles path from a known project cache home.
pub fn runtime_profiles_path_from_cache_home(cache_home: impl AsRef<Path>) -> PathBuf {
    cache_home
        .as_ref()
        .join(SEMANTIC_AGENT_PROTOCOL_RUNTIME_DIR)
        .join("profiles.json")
}

fn project_cache_root(project_root: impl AsRef<Path>) -> Result<PathBuf, String> {
    project_cache_root_with_env(project_root.as_ref(), ProjectRuntimeEnv::from_process())
}

fn project_cache_root_with_env(
    project_root: &Path,
    runtime_env: ProjectRuntimeEnv,
) -> Result<PathBuf, String> {
    if let Some(cache_root) = runtime_env.prj_cache_home {
        return Ok(cache_root);
    }

    let git_toplevel = local_git_toplevel(project_root)
        .or_else(|| command_git_toplevel(project_root))
        .ok_or_else(|| {
            format!(
                "failed to locate ASP state root: set {PRJ_CACHE_HOME_ENV} or run from a git worktree rooted above {}",
                project_root.display()
            )
        })?;

    Ok(git_toplevel.join(".cache"))
}

fn env_path(name: &str) -> Option<PathBuf> {
    env::var_os(name)
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
}

fn canonicalize_if_possible(path: PathBuf) -> PathBuf {
    path.canonicalize().unwrap_or(path)
}

fn local_git_toplevel(project_root: &Path) -> Option<PathBuf> {
    project_root
        .ancestors()
        .find(|ancestor| ancestor.join(".git").exists())
        .map(Path::to_path_buf)
}

fn command_git_toplevel(project_root: &Path) -> Option<PathBuf> {
    let output = Command::new("git")
        .arg("-C")
        .arg(project_root)
        .arg("rev-parse")
        .arg("--show-toplevel")
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let path = String::from_utf8(output.stdout).ok()?;
    let path = path.trim();
    if path.is_empty() {
        None
    } else {
        Some(PathBuf::from(path))
    }
}

#[cfg(test)]
#[path = "../tests/unit/runtime_layout.rs"]
mod runtime_layout_tests;
