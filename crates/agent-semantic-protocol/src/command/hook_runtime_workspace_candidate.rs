//! Resolves the source scope admitted by a hook lifecycle event.

use std::path::{Path, PathBuf};

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
