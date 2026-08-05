//! Resolves the source scope admitted by a hook lifecycle event.

use std::path::{Path, PathBuf};

pub(in crate::command::hook_runtime) fn direct_read_policy_project_root(
    payload: &serde_json::Value,
) -> Option<PathBuf> {
    let tool_name = payload
        .get("tool_name")
        .or_else(|| payload.get("toolName"))
        .and_then(serde_json::Value::as_str)?;
    let tool_input = payload
        .get("tool_input")
        .or_else(|| payload.get("toolInput"))?;
    agent_semantic_hook::direct_source_read_paths(tool_name, tool_input)?;
    let root = payload
        .get("cwd")
        .and_then(serde_json::Value::as_str)
        .filter(|cwd| !cwd.trim().is_empty())
        .map(PathBuf::from)
        .or_else(|| std::env::current_dir().ok())
        .unwrap_or_else(|| PathBuf::from("."));
    Some(std::fs::canonicalize(&root).unwrap_or(root))
}

pub(in crate::command::hook_runtime) fn activation_repair_project_root(
    payload: &serde_json::Value,
    activation_path: &Path,
) -> Result<PathBuf, String> {
    let payload_root = payload
        .get("cwd")
        .and_then(serde_json::Value::as_str)
        .map(PathBuf::from)
        .map(|root| std::fs::canonicalize(&root).unwrap_or(root));
    let activation_owner =
        agent_semantic_runtime::state::project_root_for_activation_path(activation_path)
            .map(|root| std::fs::canonicalize(&root).unwrap_or(root));
    if let Some(owner) = activation_owner {
        if let Some(payload_root) = payload_root
            && !payload_root.starts_with(&owner)
        {
            return Err(format!(
                "identity/state mismatch: activationOwner={} payloadCwd={}",
                owner.display(),
                payload_root.display()
            ));
        }
        return Ok(owner);
    }
    if let Some(payload_root) = payload_root {
        return Ok(payload_root);
    }
    let current = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    Ok(std::fs::canonicalize(&current).unwrap_or(current))
}

pub(in crate::command::hook_runtime) fn hook_workspace_candidate(
    payload: &serde_json::Value,
    project_root: &Path,
) -> PathBuf {
    let payload_cwd = payload
        .get("cwd")
        .and_then(serde_json::Value::as_str)
        .map(PathBuf::from);
    let workdir = payload
        .get("tool_input")
        .and_then(|tool_input| tool_input.get("workdir"))
        .and_then(serde_json::Value::as_str)
        .map(PathBuf::from);
    let command_root = match workdir {
        Some(workdir) if workdir.is_absolute() => workdir,
        Some(workdir) => payload_cwd
            .clone()
            .unwrap_or_else(|| project_root.to_path_buf())
            .join(workdir),
        None => payload_cwd.unwrap_or_else(|| project_root.to_path_buf()),
    };
    explicit_asp_workspace(payload, &command_root).unwrap_or(command_root)
}

fn explicit_asp_workspace(payload: &serde_json::Value, command_root: &Path) -> Option<PathBuf> {
    super::hook_runtime_agent_session::payload_command_strings(payload)
        .into_iter()
        .find_map(|command| asp_workspace_from_command(&command, command_root))
}

pub(in crate::command) fn requests_explicit_asp_workspace(payload: &serde_json::Value) -> bool {
    super::hook_runtime_agent_session::payload_command_strings(payload)
        .into_iter()
        .any(|command| {
            let tokens = agent_semantic_hook::semantic_shell_tokens(&command);
            let Some(asp_position) = tokens.iter().position(|token| {
                Path::new(token).file_name().and_then(|name| name.to_str()) == Some("asp")
            }) else {
                return false;
            };
            tokens[asp_position + 1..].iter().any(|token| {
                token == "--workspace"
                    || token
                        .strip_prefix("--workspace=")
                        .is_some_and(|value| !value.is_empty())
            })
        })
}

fn asp_workspace_from_command(command: &str, command_root: &Path) -> Option<PathBuf> {
    let tokens = agent_semantic_hook::semantic_shell_tokens(command);
    let asp_position = tokens.iter().position(|token| {
        Path::new(token).file_name().and_then(|name| name.to_str()) == Some("asp")
    })?;
    let workspace = tokens[asp_position + 1..]
        .windows(2)
        .find(|window| window[0] == "--workspace")
        .map(|window| PathBuf::from(&window[1]))
        .or_else(|| {
            tokens[asp_position + 1..].iter().find_map(|token| {
                token
                    .strip_prefix("--workspace=")
                    .filter(|value| !value.is_empty())
                    .map(PathBuf::from)
            })
        })?;
    Some(if workspace.is_absolute() {
        workspace
    } else {
        command_root.join(workspace)
    })
}

#[cfg(test)]
#[path = "../../tests/unit/command/hook_runtime_direct_read_policy.rs"]
mod direct_read_policy_tests;
