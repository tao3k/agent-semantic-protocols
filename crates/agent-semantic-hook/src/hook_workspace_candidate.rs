//! Parser-backed workspace projection for Hook lifecycle events.

use std::path::Component;
use std::path::Path;
use std::path::PathBuf;

/// Resolve the workspace identity carried by a Hook payload without filesystem I/O.
pub fn hook_workspace_candidate(payload: &serde_json::Value, project_root: &Path) -> PathBuf {
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
    normalize_workspace_path(
        &explicit_asp_workspace(payload, &command_root).unwrap_or(command_root),
    )
}

/// Normalize lexical workspace aliases to the same control-plane identity.
pub fn normalize_workspace_path(path: &Path) -> PathBuf {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                normalized.pop();
            }
            Component::Normal(value) => normalized.push(value),
            Component::RootDir | Component::Prefix(_) => normalized.push(component.as_os_str()),
        }
    }
    normalized
}

fn explicit_asp_workspace(payload: &serde_json::Value, command_root: &Path) -> Option<PathBuf> {
    payload_command_strings(payload)
        .into_iter()
        .find_map(|command| asp_workspace_from_command(&command, command_root))
}

fn payload_command_strings(payload: &serde_json::Value) -> Vec<String> {
    fn collect(value: &serde_json::Value, commands: &mut Vec<String>) {
        match value {
            serde_json::Value::Array(values) => {
                for value in values {
                    collect(value, commands);
                }
            }
            serde_json::Value::Object(map) => {
                for (key, value) in map {
                    match (key.as_str(), value) {
                        ("command" | "cmd" | "script", serde_json::Value::String(command))
                            if !command.trim().is_empty() =>
                        {
                            commands.push(command.clone());
                        }
                        ("command" | "cmd", serde_json::Value::Array(parts)) => {
                            let command = parts
                                .iter()
                                .filter_map(serde_json::Value::as_str)
                                .collect::<Vec<_>>()
                                .join(" ");
                            if !command.trim().is_empty() {
                                commands.push(command);
                            }
                        }
                        _ => collect(value, commands),
                    }
                }
            }
            _ => {}
        }
    }

    let mut commands = Vec::new();
    collect(payload, &mut commands);
    commands.sort();
    commands.dedup();
    commands
}

fn asp_workspace_from_command(command: &str, command_root: &Path) -> Option<PathBuf> {
    let tokens = crate::semantic_shell_tokens(command);
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
