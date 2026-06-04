//! Cache path helpers for `agent-semantic-protocol` hook state.

use std::env;
use std::path::{Path, PathBuf};
use std::process::Command;

const PRJ_HOME_CACHE_ENV: &str = "PRJ_HOME_CACHE";
const SEMANTIC_AGENT_PROTOCOL_HOOK_DIR: &str = "agent-semantic-protocol/hooks";

/// Resolve the project cache directory for hook activation and profiles.
pub fn project_hook_cache_dir(project_root: impl AsRef<Path>) -> Result<PathBuf, String> {
    Ok(project_cache_root(project_root)?.join(SEMANTIC_AGENT_PROTOCOL_HOOK_DIR))
}

/// Resolve the project cache directory used for hook event state.
pub fn project_hook_state_dir(project_root: impl AsRef<Path>) -> Result<PathBuf, String> {
    project_hook_cache_dir(project_root)
}

fn project_cache_root(project_root: impl AsRef<Path>) -> Result<PathBuf, String> {
    if let Some(cache_root) = prj_home_cache_dir() {
        return Ok(cache_root);
    }

    let project_root = project_root.as_ref();
    let git_toplevel = local_git_toplevel(project_root)
        .or_else(|| command_git_toplevel(project_root))
        .ok_or_else(|| {
            format!(
                "failed to locate hook cache root: set {PRJ_HOME_CACHE_ENV} or run from a git worktree rooted above {}",
                project_root.display()
            )
        })?;

    Ok(git_toplevel.join(".cache"))
}

fn prj_home_cache_dir() -> Option<PathBuf> {
    env::var_os(PRJ_HOME_CACHE_ENV)
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
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
