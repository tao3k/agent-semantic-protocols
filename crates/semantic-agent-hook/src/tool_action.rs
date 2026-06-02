use serde_json::Value;

use crate::command::{path_like_tokens, semantic_shell_tokens};
use crate::protocol::DecisionSubject;

#[derive(Clone, Debug)]
pub(crate) struct ToolAction {
    pub(crate) tool_name: String,
    pub(crate) command: Option<String>,
    pub(crate) paths: Vec<String>,
}

pub(crate) fn payload_string(payload: &Value, key: &str) -> Option<String> {
    payload.get(key).and_then(Value::as_str).map(str::to_string)
}

pub(crate) fn collect_tool_actions(tool_name: &str, tool_input: &Value) -> Vec<ToolAction> {
    let command = extract_command_direct(tool_name, tool_input);
    let mut paths = extract_paths_direct(tool_input);
    if let Some(command) = command.as_deref() {
        for path in command_source_paths(command) {
            if !paths.iter().any(|existing| existing == &path) {
                paths.push(path);
            }
        }
    }
    let mut actions = vec![ToolAction {
        tool_name: tool_name.to_string(),
        command,
        paths,
    }];
    for nested in nested_tool_actions(tool_input) {
        actions.extend(collect_tool_actions(&nested.tool_name, nested.input));
    }
    actions
}

pub(crate) fn subject_for_action(action: &ToolAction) -> DecisionSubject {
    DecisionSubject {
        tool_name: if action.tool_name.is_empty() {
            None
        } else {
            Some(action.tool_name.clone())
        },
        command: action.command.clone(),
        paths: action.paths.clone(),
    }
}

fn extract_command_direct(tool_name: &str, tool_input: &Value) -> Option<String> {
    if !matches!(
        tool_name,
        "functions.exec_command" | "exec_command" | "command_execution" | "Bash" | "Shell"
    ) {
        return None;
    }
    for key in ["cmd", "command"] {
        if let Some(command) = tool_input.get(key).and_then(Value::as_str) {
            return Some(command.to_string());
        }
    }
    if tool_name == "command_execution" {
        return tool_input
            .get("tool_input")
            .and_then(|value| value.get("command"))
            .and_then(Value::as_str)
            .map(str::to_string);
    }
    None
}

fn extract_paths_direct(tool_input: &Value) -> Vec<String> {
    let mut paths = Vec::new();
    for key in ["path", "file_path", "filePath"] {
        if let Some(path) = tool_input.get(key).and_then(Value::as_str) {
            paths.push(path.to_string());
        }
    }
    if let Some(array) = tool_input.get("paths").and_then(Value::as_array) {
        for value in array {
            if let Some(path) = value.as_str() {
                paths.push(path.to_string());
            }
        }
    }
    paths
}

struct NestedToolAction<'a> {
    tool_name: String,
    input: &'a Value,
}

fn nested_tool_actions(tool_input: &Value) -> Vec<NestedToolAction<'_>> {
    let mut nested = Vec::new();
    for key in ["tool_uses", "toolUses"] {
        let Some(tool_uses) = tool_input.get(key).and_then(Value::as_array) else {
            continue;
        };
        for tool_use in tool_uses {
            let Some(tool_name) = payload_string(tool_use, "recipient_name")
                .or_else(|| payload_string(tool_use, "recipientName"))
                .or_else(|| payload_string(tool_use, "tool_name"))
                .or_else(|| payload_string(tool_use, "toolName"))
            else {
                continue;
            };
            let input = tool_use
                .get("parameters")
                .or_else(|| tool_use.get("tool_input"))
                .or_else(|| tool_use.get("toolInput"))
                .unwrap_or(&Value::Null);
            nested.push(NestedToolAction { tool_name, input });
        }
    }
    nested
}

fn command_source_paths(command: &str) -> Vec<String> {
    let tokens = semantic_shell_tokens(command);
    path_like_tokens(&tokens)
        .into_iter()
        .map(str::to_string)
        .collect()
}
