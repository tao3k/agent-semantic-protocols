#![deny(dead_code)]

//! Unified project identity, configuration, and local state layout for ASP.

use std::env;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Canonical environment variable for project-local ASP cache state.
pub const PRJ_CACHE_HOME_ENV: &str = "PRJ_CACHE_HOME";

const SEMANTIC_AGENT_PROTOCOL_DIR: &str = "agent-semantic-protocol";
const SEMANTIC_AGENT_PROTOCOL_HOOK_DIR: &str = "agent-semantic-protocol/hooks";
const SEMANTIC_AGENT_PROTOCOL_CLIENT_DIR: &str = "agent-semantic-protocol/client";
const SEMANTIC_AGENT_PROTOCOL_RUNTIME_DIR: &str = "agent-semantic-protocol/runtime";
const SEMANTIC_AGENT_PROTOCOL_RUNTIME_BIN_DIR: &str = "agent-semantic-protocol/runtime/bin";
const SEMANTIC_AGENT_PROTOCOL_PROVIDER_LOCK_DIR: &str = "agent-semantic-protocol/runtime/providers";

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

/// Whether PRJ* style project environment variables are meaningful for a root.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProjectEnvStatus {
    DirenvAtGitToplevel { envrc_path: PathBuf },
    Unavailable,
}

/// Read-only project runtime layout derived from git and ASP cache conventions.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectRuntimeLayout {
    /// Input root used by the caller before git normalization.
    pub requested_root: PathBuf,
    /// Git toplevel for this project identity.
    pub git_toplevel: Option<PathBuf>,
    /// Project home used for PRJ* style project variables.
    pub project_home: Option<PathBuf>,
    /// Whether project env variables are trustworthy for this project.
    pub project_env: ProjectEnvStatus,
    /// Cache home used for state storage.
    pub cache_home: Option<PathBuf>,
    /// Source that selected `cache_home`.
    pub cache_source: Option<ProjectCacheSource>,
    /// Canonical project cache override value.
    pub prj_cache_home: Option<PathBuf>,
    /// Protocol root below `cache_home`.
    pub protocol_home: Option<PathBuf>,
    /// Hook activation and event-state directory.
    pub hook_cache_dir: Option<PathBuf>,
    /// Hook append-only event-state directory.
    pub hook_state_dir: Option<PathBuf>,
    /// Hook activation path.
    pub activation_path: Option<PathBuf>,
    /// Client manifest and DB Engine directory.
    pub client_cache_dir: Option<PathBuf>,
    /// Provider/client artifact directory.
    ///
    /// State Core owns the v2 artifact root. Config runtime layout does not
    /// expose a project-local artifact authority.
    pub artifacts_dir: Option<PathBuf>,
    /// Runtime command-shim directory.
    pub runtime_home: Option<PathBuf>,
    /// Runtime command-shim binary directory.
    pub runtime_bin_dir: Option<PathBuf>,
    /// Runtime provider release lock directory.
    pub provider_lock_dir: Option<PathBuf>,
    /// Agent skill/config directory under git toplevel.
    pub agents_dir: Option<PathBuf>,
    /// Installed ASP Org skill path under `.agents`.
    pub agent_skill_path: Option<PathBuf>,
}

/// Injected environment for deterministic config tests and controlled callers.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ProjectRuntimeEnv {
    pub prj_cache_home: Option<PathBuf>,
}

impl ProjectRuntimeEnv {
    fn from_process() -> Self {
        Self {
            prj_cache_home: env_path(PRJ_CACHE_HOME_ENV),
        }
    }
}

/// Resolve all project-local runtime paths without creating or refreshing state.
pub fn project_runtime_layout(project_root: impl AsRef<Path>) -> ProjectRuntimeLayout {
    let project_root = project_root.as_ref();
    project_runtime_layout_with_env(project_root, ProjectRuntimeEnv::from_process())
}

/// Resolve project-local runtime paths with an injected environment.
pub fn project_runtime_layout_with_env(
    project_root: &Path,
    runtime_env: ProjectRuntimeEnv,
) -> ProjectRuntimeLayout {
    let requested_root = project_root.to_path_buf();
    let git_toplevel = local_git_toplevel(project_root)
        .or_else(|| command_git_toplevel(project_root))
        .map(canonicalize_if_possible);
    let project_home = git_toplevel.clone();
    let project_env = project_env_status(git_toplevel.as_deref());
    let prj_cache_home = runtime_env.prj_cache_home;

    let (cache_home, cache_source) = if let Some(git_toplevel) = git_toplevel.as_ref() {
        (
            Some(git_toplevel.join(".cache")),
            Some(ProjectCacheSource::GitToplevel),
        )
    } else if let Some(cache_root) = prj_cache_home.clone() {
        (Some(cache_root), Some(ProjectCacheSource::PrjCacheHome))
    } else {
        (None, None)
    };

    let protocol_home = cache_home
        .as_ref()
        .map(|cache_home| cache_home.join(SEMANTIC_AGENT_PROTOCOL_DIR));
    let hook_cache_dir = cache_home
        .as_ref()
        .map(|cache_home| cache_home.join(SEMANTIC_AGENT_PROTOCOL_HOOK_DIR));
    let hook_state_dir: Option<PathBuf> = None;
    let activation_path = hook_cache_dir
        .as_ref()
        .map(|hook_cache_dir| hook_cache_dir.join("activation.json"));
    let client_cache_dir = cache_home
        .as_ref()
        .map(|cache_home| cache_home.join(SEMANTIC_AGENT_PROTOCOL_CLIENT_DIR));
    let artifacts_dir = None;
    let runtime_home = cache_home
        .as_ref()
        .map(|cache_home| cache_home.join(SEMANTIC_AGENT_PROTOCOL_RUNTIME_DIR));
    let runtime_bin_dir = cache_home
        .as_ref()
        .map(|cache_home| cache_home.join(SEMANTIC_AGENT_PROTOCOL_RUNTIME_BIN_DIR));
    let provider_lock_dir = cache_home
        .as_ref()
        .map(|cache_home| cache_home.join(SEMANTIC_AGENT_PROTOCOL_PROVIDER_LOCK_DIR));
    let agents_dir = git_toplevel
        .as_ref()
        .map(|git_toplevel| git_toplevel.join(".agents"));
    let agent_skill_path = agents_dir
        .as_ref()
        .map(|agents_dir| agents_dir.join("skills/agent-semantic-protocols/SKILL.org"));

    ProjectRuntimeLayout {
        requested_root,
        git_toplevel,
        project_home,
        project_env,
        cache_home,
        cache_source,
        prj_cache_home,
        protocol_home,
        hook_cache_dir,
        hook_state_dir,
        activation_path,
        client_cache_dir,
        artifacts_dir,
        runtime_home,
        runtime_bin_dir,
        provider_lock_dir,
        agents_dir,
        agent_skill_path,
    }
}

fn project_env_status(git_toplevel: Option<&Path>) -> ProjectEnvStatus {
    let Some(git_toplevel) = git_toplevel else {
        return ProjectEnvStatus::Unavailable;
    };
    let envrc_path = git_toplevel.join(".envrc");
    if envrc_path.is_file() {
        ProjectEnvStatus::DirenvAtGitToplevel { envrc_path }
    } else {
        ProjectEnvStatus::Unavailable
    }
}

/// Resolve the project cache directory for hook activation and profiles.
pub fn project_hook_cache_dir(project_root: impl AsRef<Path>) -> Result<PathBuf, String> {
    Ok(project_cache_root(project_root)?.join(SEMANTIC_AGENT_PROTOCOL_HOOK_DIR))
}

/// Resolve the project-local ASP protocol home under the cache root.
pub fn project_protocol_home(project_root: impl AsRef<Path>) -> Result<PathBuf, String> {
    Ok(project_cache_root(project_root)?.join(SEMANTIC_AGENT_PROTOCOL_DIR))
}

/// Resolve the managed hook activation file path.
pub fn project_activation_path(project_root: impl AsRef<Path>) -> Result<PathBuf, String> {
    Ok(project_hook_cache_dir(project_root)?.join("activation.json"))
}

/// Return the agent semantic client cache directory for an activated project.
pub fn project_client_cache_dir(project_root: impl AsRef<Path>) -> Result<PathBuf, String> {
    Ok(project_cache_root(project_root)?.join(SEMANTIC_AGENT_PROTOCOL_CLIENT_DIR))
}

/// Return the runtime command-shim directory for provider execution.
pub fn project_runtime_bin_dir(project_root: impl AsRef<Path>) -> Result<PathBuf, String> {
    Ok(project_cache_root(project_root)?.join(SEMANTIC_AGENT_PROTOCOL_RUNTIME_BIN_DIR))
}

/// Return the runtime provider release lock directory.
pub fn project_provider_lock_dir(project_root: impl AsRef<Path>) -> Result<PathBuf, String> {
    Ok(project_cache_root(project_root)?.join(SEMANTIC_AGENT_PROTOCOL_PROVIDER_LOCK_DIR))
}

/// Return the state cache root, using git toplevel before `PRJ_CACHE_HOME`.
pub fn project_cache_root(project_root: impl AsRef<Path>) -> Result<PathBuf, String> {
    project_cache_root_with_env(project_root.as_ref(), ProjectRuntimeEnv::from_process())
}

/// Return the state cache root with an injected environment.
pub fn project_cache_root_with_env(
    project_root: &Path,
    runtime_env: ProjectRuntimeEnv,
) -> Result<PathBuf, String> {
    if let Some(git_toplevel) =
        local_git_toplevel(project_root).or_else(|| command_git_toplevel(project_root))
    {
        return Ok(git_toplevel.join(".cache"));
    }

    runtime_env.prj_cache_home.ok_or_else(|| {
        format!(
            "failed to locate ASP state root: run from a git worktree rooted above {} or set {PRJ_CACHE_HOME_ENV}",
            project_root.display()
        )
    })
}

fn env_path(name: &str) -> Option<PathBuf> {
    env::var_os(name)
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .map(canonicalize_if_possible)
}

fn canonicalize_if_possible(path: PathBuf) -> PathBuf {
    path.canonicalize().unwrap_or(path)
}

fn local_git_toplevel(project_root: &Path) -> Option<PathBuf> {
    project_root
        .ancestors()
        .find(|ancestor| ancestor.join(".git").exists())
        .map(Path::to_path_buf)
        .map(canonicalize_if_possible)
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
        Some(canonicalize_if_possible(PathBuf::from(path)))
    }
}
