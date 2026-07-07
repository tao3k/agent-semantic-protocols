//! Project state synchronization for `asp sync`.

use super::org_capture::{org_artifacts_root_for_project, run_org_state_sync};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

pub(crate) fn run_sync_command(args: &[String]) -> Result<(), String> {
    if args
        .iter()
        .any(|arg| matches!(arg.as_str(), "help" | "--help" | "-h"))
    {
        println!("{}", usage());
        return Ok(());
    }
    let project_root = project_root_arg(args)?;
    let sync = run_org_state_sync(&project_root)?;
    let agent_config_count = sync_global_agent_configs()?;
    sync_codex_plugin_activation_cache(&project_root)?;
    let org_state = agent_semantic_runtime::project_state_paths(&project_root)?
        .protocol_home
        .join("org");
    let org_artifacts = org_artifacts_root_for_project(&project_root)?;
    println!(
        "[asp-sync] orgState={} orgArtifacts={} orgRepo={} orgStatus={} agentConfigs={}",
        display_path(&project_root, &org_state),
        display_path(&project_root, &org_artifacts),
        sync.source,
        sync.status,
        agent_config_count,
    );
    Ok(())
}

fn sync_global_agent_configs() -> Result<usize, String> {
    let source_dir = agent_semantic_runtime::state_core::resolve_state_home()?.join("agents");
    if !source_dir.exists() {
        return Ok(0);
    }
    let mut synced = 0usize;
    for entry in fs::read_dir(&source_dir)
        .map_err(|error| format!("failed to read {}: {error}", source_dir.display()))?
    {
        let entry = entry.map_err(|error| {
            format!("failed to read entry in {}: {error}", source_dir.display())
        })?;
        let source = entry.path();
        if !source.is_file() {
            continue;
        }
        let Some(file_name) = source.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        if let Some(agent_name) = file_name.strip_suffix("_codex.toml") {
            let target = codex_home()
                .join("agents")
                .join(format!("{agent_name}.toml"));
            project_agent_config(&source, &target)?;
            synced += 1;
        } else if let Some(agent_name) = file_name.strip_suffix("_claude.md") {
            let target = claude_home()
                .join("agents")
                .join(format!("{agent_name}.md"));
            project_agent_config(&source, &target)?;
            synced += 1;
        } else if let Some(agent_name) = file_name.strip_suffix("_claude.toml") {
            let target = claude_home()
                .join("agents")
                .join(format!("{agent_name}.toml"));
            project_agent_config(&source, &target)?;
            synced += 1;
        }
    }
    Ok(synced)
}

fn sync_codex_plugin_activation_cache(project_root: &Path) -> Result<(), String> {
    let source = agent_semantic_runtime::project_state_paths(project_root)?.activation_path;
    if !source.is_file() {
        return Ok(());
    }
    let target = codex_home()
        .join(".cache")
        .join("agent-semantic-protocol")
        .join("hooks")
        .join("activation.json");
    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("failed to create {}: {error}", parent.display()))?;
    }
    fs::copy(&source, &target).map(|_| ()).map_err(|error| {
        format!(
            "failed to copy {} to {}: {error}",
            source.display(),
            target.display()
        )
    })
}

fn codex_home() -> PathBuf {
    env::var_os("CODEX_HOME")
        .map(PathBuf::from)
        .or_else(|| env::var_os("HOME").map(|home| PathBuf::from(home).join(".codex")))
        .unwrap_or_else(|| PathBuf::from(".codex"))
}

fn claude_home() -> PathBuf {
    env::var_os("CLAUDE_HOME")
        .map(PathBuf::from)
        .or_else(|| env::var_os("HOME").map(|home| PathBuf::from(home).join(".claude")))
        .unwrap_or_else(|| PathBuf::from(".claude"))
}

fn project_agent_config(source: &Path, target: &Path) -> Result<(), String> {
    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("failed to create {}: {error}", parent.display()))?;
    }
    match fs::symlink_metadata(target) {
        Ok(metadata) => {
            if metadata.is_dir() {
                return Err(format!("cannot replace directory {}", target.display()));
            }
            fs::remove_file(target)
                .map_err(|error| format!("failed to replace {}: {error}", target.display()))?;
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => {
            return Err(format!("failed to inspect {}: {error}", target.display()));
        }
    }
    link_or_copy_agent_config(source, target)
}

#[cfg(unix)]
fn link_or_copy_agent_config(source: &Path, target: &Path) -> Result<(), String> {
    std::os::unix::fs::symlink(source, target).map_err(|error| {
        format!(
            "failed to symlink {} -> {}: {error}",
            target.display(),
            source.display()
        )
    })
}

#[cfg(not(unix))]
fn link_or_copy_agent_config(source: &Path, target: &Path) -> Result<(), String> {
    fs::copy(source, target).map(|_| ()).map_err(|error| {
        format!(
            "failed to copy {} -> {}: {error}",
            source.display(),
            target.display()
        )
    })
}

fn project_root_arg(args: &[String]) -> Result<PathBuf, String> {
    let cwd = env::current_dir().map_err(|error| format!("failed to read current dir: {error}"))?;
    let root = args
        .iter()
        .find(|arg| !arg.starts_with('-'))
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));
    Ok(if root.is_absolute() {
        root
    } else {
        cwd.join(root)
    })
}

fn display_path(project_root: &Path, path: &Path) -> String {
    path.strip_prefix(project_root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

fn usage() -> &'static str {
    "usage: asp sync [PROJECT_ROOT]\n\nSynchronizes project-owned ASP state. The Org resource tree is cloned or fast-forwarded from ASP_ORG_REPO_URL, defaulting to https://github.com/tao3k/org.git. Agent-authored Org state belongs under the root returned by `asp paths --get orgArtifacts [PROJECT_ROOT]`.\n\nAlso refreshes ASP-owned global agent config projections from ~/.agent-semantic-protocols/agents/*_codex.toml and *_claude.{md,toml} into the host agent directories."
}
