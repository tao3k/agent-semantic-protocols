//! Normalizes platform tool payloads into hook classifier actions.

//! Converts client tool payloads into action-level source access intents.

use std::borrow::Cow;

use serde_json::Value;

use crate::command::{apply_patch_source_paths, command_source_paths, semantic_shell_tokens};
use crate::protocol::DecisionSubject;

const PATH_SCALAR_KEYS: &[&str] = &[
    "path",
    "file",
    "file_name",
    "fileName",
    "file_path",
    "filePath",
    "absolute_path",
    "absolutePath",
    "relative_path",
    "relativePath",
    "uri",
];
const PATH_CONTAINER_KEYS: &[&str] = &[
    "paths",
    "files",
    "resource",
    "resources",
    "uris",
    "document",
    "documents",
    "text_document",
    "textDocument",
];
const PATH_VALUE_KEYS: &[&str] = &[
    "path",
    "file",
    "file_name",
    "fileName",
    "file_path",
    "filePath",
    "absolute_path",
    "absolutePath",
    "relative_path",
    "relativePath",
    "uri",
    "paths",
    "files",
    "resource",
    "resources",
    "uris",
    "document",
    "documents",
    "text_document",
    "textDocument",
];
const ACTION_SCAN_KEYS: &[&str] = &[
    "commandActions",
    "command_actions",
    "action",
    "toolAction",
    "tool_action",
    "item",
    "items",
    "input",
    "arguments",
    "args",
    "parameters",
    "params",
    "toolInput",
    "tool_input",
    "toolUse",
    "tool_use",
    "function",
    "tool_uses",
    "toolUses",
    "tools",
    "tool_calls",
    "toolCalls",
];

#[derive(Clone, Debug)]
pub(crate) struct ToolAction {
    pub(crate) tool_name: String,
    pub(crate) surface: ToolSurface,
    pub(crate) operation: OperationIntent,
    pub(crate) command: Option<String>,
    pub(crate) command_tokens: Option<Vec<String>>,
    pub(crate) paths: Vec<String>,
}

impl ToolAction {
    pub(crate) fn command_tokens(&self) -> Option<Cow<'_, [String]>> {
        self.command_tokens
            .as_deref()
            .map(Cow::Borrowed)
            .or_else(|| {
                self.command
                    .as_deref()
                    .map(|command| Cow::Owned(semantic_shell_tokens(command)))
            })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ToolSurface {
    CodexApplyPatch,
    CodexDirectRead,
    CodexDirectoryRead,
    CodexFuzzyFileSearch,
    CodexMcpRead,
    CodexNestedTools,
    CodexShell,
    CodexStdinContinuation,
    Unknown,
}

impl ToolSurface {
    pub(crate) fn from_tool_name(tool_name: &str) -> Self {
        let lower = tool_name.to_ascii_lowercase();
        if lower.starts_with("mcp__") && lower.contains("__read") {
            return Self::CodexMcpRead;
        }
        let normalized = lower
            .chars()
            .map(|ch| match ch {
                '-' | '/' | ':' => '.',
                _ => ch,
            })
            .collect::<String>();
        let leaf = normalized
            .split('.')
            .next_back()
            .unwrap_or(normalized.as_str());
        match normalized.as_str() {
            "edit"
            | "multiedit"
            | "write"
            | "notebookedit"
            | "fswritefile"
            | "fsremove"
            | "fscopy"
            | "fsrename"
            | "functions.edit"
            | "functions.multiedit"
            | "functions.write"
            | "functions.notebookedit" => Self::CodexApplyPatch,
            "apply_patch" | "applypatch" => Self::CodexApplyPatch,
            "bash" | "shell" | "functions.exec_command" | "exec_command" | "command_execution" => {
                Self::CodexShell
            }
            "grep" | "glob" => Self::CodexFuzzyFileSearch,
            "multi_tool_use.parallel" => Self::CodexNestedTools,
            "write_stdin" | "writestdin" | "process.write_stdin" | "process.writestdin" => {
                Self::CodexStdinContinuation
            }
            "fuzzyfilesearch"
            | "fuzzyfilesearch.sessionstart"
            | "fuzzyfilesearch.sessionupdate" => Self::CodexFuzzyFileSearch,
            _ if matches!(leaf, "read" | "readfile" | "read_file" | "fsreadfile") => {
                Self::CodexDirectRead
            }
            _ if matches!(leaf, "readdirectory" | "read_directory" | "fsreaddirectory") => {
                Self::CodexDirectoryRead
            }
            _ if matches!(
                leaf,
                "write"
                    | "writefile"
                    | "write_file"
                    | "remove"
                    | "copy"
                    | "rename"
                    | "fswritefile"
                    | "fsremove"
                    | "fscopy"
                    | "fsrename"
            ) =>
            {
                Self::CodexApplyPatch
            }
            _ if normalized.ends_with(".apply_patch") => Self::CodexApplyPatch,
            _ => Self::Unknown,
        }
    }

    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::CodexApplyPatch => "apply-patch",
            Self::CodexDirectRead => "direct-read",
            Self::CodexDirectoryRead => "directory-read",
            Self::CodexFuzzyFileSearch => "fuzzy-file-search",
            Self::CodexMcpRead => "mcp-read",
            Self::CodexNestedTools => "nested-tools",
            Self::CodexShell => "shell-command",
            Self::CodexStdinContinuation => "stdin-continuation",
            Self::Unknown => "unknown",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum OperationIntent {
    ApplyPatch,
    DirectoryRead,
    DirectRead,
    FileSearch,
    NestedTools,
    ShellCommand,
    StdinContinuation,
    Unknown,
}

impl OperationIntent {
    pub(crate) fn from_action(
        surface: ToolSurface,
        command: Option<&str>,
        paths: &[String],
    ) -> Self {
        match surface {
            ToolSurface::CodexApplyPatch => Self::ApplyPatch,
            ToolSurface::CodexDirectRead | ToolSurface::CodexMcpRead => Self::DirectRead,
            ToolSurface::CodexDirectoryRead => Self::DirectoryRead,
            ToolSurface::CodexFuzzyFileSearch => Self::FileSearch,
            ToolSurface::CodexNestedTools => Self::NestedTools,
            ToolSurface::CodexShell if command.is_some() => Self::ShellCommand,
            ToolSurface::CodexStdinContinuation if command.is_some() => Self::StdinContinuation,
            ToolSurface::Unknown if command.is_none() && !paths.is_empty() => Self::DirectRead,
            _ => Self::Unknown,
        }
    }

    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::ApplyPatch => "apply-patch",
            Self::DirectoryRead => "directory-read",
            Self::DirectRead => "direct-read",
            Self::FileSearch => "file-search",
            Self::NestedTools => "nested-tools",
            Self::ShellCommand => "shell-command",
            Self::StdinContinuation => "stdin-continuation",
            Self::Unknown => "unknown",
        }
    }
}

pub(crate) fn payload_string(payload: &Value, key: &str) -> Option<String> {
    payload.get(key).and_then(Value::as_str).map(str::to_string)
}

/// Collects direct, shell, nested, and Codex `CommandAction` intents.
/// Extract source read/search/list/write intents from a client tool payload.
pub fn collect_tool_actions(tool_name: &str, tool_input: &Value) -> Vec<ToolAction> {
    const CODEX_COMMAND_ACTION_KEYS: &[&str] = &["commandActions", "command_actions"];
    const CODEX_DIRECT_ACTION_KEYS: &[&str] = &["action", "toolAction", "tool_action"];
    const CODEX_ACTION_CONTAINER_KEYS: &[&str] = &[
        "item",
        "items",
        "input",
        "arguments",
        "args",
        "parameters",
        "params",
        "toolInput",
        "tool_input",
        "toolUse",
        "tool_use",
    ];

    fn codex_command_actions(tool_name: &str, value: &Value) -> Vec<ToolAction> {
        let mut actions = Vec::new();
        collect_codex_command_actions(tool_name, value, &mut actions, false);
        actions
    }

    fn collect_codex_command_actions(
        tool_name: &str,
        value: &Value,
        actions: &mut Vec<ToolAction>,
        direct_action: bool,
    ) {
        if direct_action && let Some(action) = codex_command_action(tool_name, value) {
            push_unique_action(actions, action);
            return;
        }

        let Some(object) = value.as_object() else {
            return;
        };

        for key in CODEX_COMMAND_ACTION_KEYS {
            if let Some(command_actions) = object.get(*key) {
                collect_codex_command_action_values(tool_name, command_actions, actions);
            }
        }

        for key in CODEX_DIRECT_ACTION_KEYS {
            if let Some(action) = object.get(*key) {
                collect_codex_command_actions(tool_name, action, actions, true);
            }
        }
        for key in CODEX_ACTION_CONTAINER_KEYS {
            if let Some(value) = object.get(*key) {
                collect_codex_item_actions(tool_name, value, actions);
            }
        }

        if is_codex_command_execution_tool(tool_name)
            && let Some(action) = codex_command_action(tool_name, value)
        {
            push_unique_action(actions, action);
        }
    }

    fn collect_codex_command_action_values(
        tool_name: &str,
        value: &Value,
        actions: &mut Vec<ToolAction>,
    ) {
        match value {
            Value::Array(values) => {
                for value in values {
                    collect_codex_command_actions(tool_name, value, actions, true);
                }
            }
            _ => collect_codex_command_actions(tool_name, value, actions, true),
        }
    }

    fn collect_codex_item_actions(tool_name: &str, value: &Value, actions: &mut Vec<ToolAction>) {
        match value {
            Value::Array(values) => {
                for value in values {
                    collect_codex_item_actions(tool_name, value, actions);
                }
            }
            Value::Object(object) => {
                for key in CODEX_DIRECT_ACTION_KEYS {
                    if let Some(action) = object.get(*key) {
                        collect_codex_command_actions(tool_name, action, actions, true);
                    }
                }
                for key in CODEX_ACTION_CONTAINER_KEYS {
                    if let Some(value) = object.get(*key) {
                        collect_codex_item_actions(tool_name, value, actions);
                    }
                }
            }
            _ => {}
        }
    }

    fn is_codex_command_execution_tool(tool_name: &str) -> bool {
        is_codex_command_execution_tool_name(tool_name)
    }

    fn codex_command_action(tool_name: &str, value: &Value) -> Option<ToolAction> {
        let object = value.as_object()?;
        let action_type = object.get("type").and_then(Value::as_str)?;
        let command = object
            .get("command")
            .or_else(|| object.get("cmd"))
            .and_then(Value::as_str)
            .map(str::to_string);
        let mut paths = Vec::new();
        if let Some(path) = object.get("path") {
            paths.extend(path_values(path));
        }
        if paths.is_empty()
            && let Some(name) = object.get("name").and_then(Value::as_str)
        {
            push_unique_path(&mut paths, name.to_string());
        }

        let (surface, operation, command) = match action_type {
            "read" => (
                ToolSurface::CodexDirectRead,
                OperationIntent::DirectRead,
                command,
            ),
            "listFiles" | "list_files" => (
                ToolSurface::CodexDirectoryRead,
                OperationIntent::DirectoryRead,
                command,
            ),
            "search" => (
                ToolSurface::CodexShell,
                OperationIntent::ShellCommand,
                command.or_else(|| synthesize_codex_search_command(object)),
            ),
            "unknown" => (
                ToolSurface::CodexShell,
                OperationIntent::ShellCommand,
                command,
            ),
            _ => return None,
        };

        if command.is_none() && paths.is_empty() {
            return None;
        }
        let command_tokens = command.as_deref().map(semantic_shell_tokens);
        if let (Some(command), Some(tokens)) = (command.as_deref(), command_tokens.as_deref()) {
            for path in command_source_paths(command, tokens) {
                push_unique_path(&mut paths, path);
            }
        }

        Some(ToolAction {
            tool_name: format!("{tool_name}.command_action.{action_type}"),
            surface,
            operation,
            command,
            command_tokens,
            paths,
        })
    }

    fn synthesize_codex_search_command(object: &serde_json::Map<String, Value>) -> Option<String> {
        let query = object.get("query").and_then(Value::as_str)?.trim();
        if query.is_empty() {
            return None;
        }

        let mut command = format!("rg {}", shell_quote_arg(query));
        if let Some(path) = object
            .get("path")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|path| !path.is_empty())
        {
            command.push(' ');
            command.push_str(&shell_quote_arg(path));
        }
        Some(command)
    }

    fn push_unique_action(actions: &mut Vec<ToolAction>, action: ToolAction) {
        if actions.iter().any(|existing| {
            existing.tool_name == action.tool_name
                && existing.command == action.command
                && existing.paths == action.paths
                && existing.operation == action.operation
        }) {
            return;
        }
        actions.push(action);
    }

    let decoded_tool_input = decoded_json_input(tool_input);
    let tool_input = decoded_tool_input.as_ref().unwrap_or(tool_input);
    let surface = ToolSurface::from_tool_name(tool_name);
    let command = extract_command_direct(surface, tool_name, tool_input);
    let command_tokens = command.as_deref().map(semantic_shell_tokens);
    let mut paths = extract_paths_direct(tool_input);
    if let Some(command) = command.as_deref() {
        let patch_paths = apply_patch_source_paths(tool_name, command);
        let mut command_paths = if !patch_paths.is_empty() {
            patch_paths
        } else if command_paths_are_needed_at_action_boundary(surface, command_tokens.as_deref()) {
            command_tokens
                .as_deref()
                .map(|tokens| command_source_paths(command, tokens))
                .unwrap_or_default()
        } else {
            Vec::new()
        };
        append_base_paths_for_ranges(&mut command_paths);
        for path in command_paths {
            if !paths.iter().any(|existing| existing == &path) {
                paths.push(path);
            }
        }
    }
    let operation = OperationIntent::from_action(surface, command.as_deref(), &paths);
    let mut actions = vec![ToolAction {
        tool_name: tool_name.to_string(),
        surface,
        operation,
        command,
        command_tokens,
        paths,
    }];
    if tool_input_needs_action_scan(tool_name, tool_input) {
        actions.extend(codex_command_actions(tool_name, tool_input));
        for nested in nested_tool_actions(tool_input) {
            actions.extend(collect_tool_actions(&nested.tool_name, &nested.input));
        }
    }
    actions
}

fn command_paths_are_needed_at_action_boundary(
    surface: ToolSurface,
    command_tokens: Option<&[String]>,
) -> bool {
    if !matches!(
        surface,
        ToolSurface::CodexShell | ToolSurface::CodexStdinContinuation
    ) {
        return true;
    }
    let Some(command_name) = command_tokens
        .and_then(|tokens| tokens.first())
        .map(|token| token.rsplit('/').next().unwrap_or(token))
    else {
        return false;
    };
    matches!(
        command_name,
        "awk"
            | "gawk"
            | "sed"
            | "gsed"
            | "head"
            | "ghead"
            | "tail"
            | "gtail"
            | "cat"
            | "less"
            | "more"
            | "bat"
            | "nl"
            | "read"
            | "rtk"
    )
}

fn append_base_paths_for_ranges(paths: &mut Vec<String>) {
    for path in paths.clone() {
        if let Some(base) = path_without_line_range(&path)
            && !paths.iter().any(|path| path == base)
        {
            paths.push(base.to_string());
        }
    }
}

fn path_without_line_range(path: &str) -> Option<&str> {
    let (base, suffix) = path.rsplit_once(':')?;
    if suffix.chars().all(|character| character.is_ascii_digit()) {
        let (base, start) = base.rsplit_once(':')?;
        return start
            .chars()
            .all(|character| character.is_ascii_digit())
            .then_some(base);
    }
    let (start, end) = suffix.split_once('-')?;
    (!start.is_empty()
        && !end.is_empty()
        && start.chars().all(|character| character.is_ascii_digit())
        && end.chars().all(|character| character.is_ascii_digit()))
    .then_some(base)
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

fn extract_command_direct(
    surface: ToolSurface,
    tool_name: &str,
    tool_input: &Value,
) -> Option<String> {
    let normalized_tool_name = tool_name.to_ascii_lowercase();
    if surface == ToolSurface::CodexApplyPatch {
        if let Some(patch) = tool_input.as_str() {
            return Some(patch.to_string());
        }
        for key in ["patch", "input", "text", "command"] {
            if let Some(patch) = tool_input.get(key).and_then(Value::as_str) {
                return Some(patch.to_string());
            }
        }
    }
    if surface == ToolSurface::CodexStdinContinuation {
        return tool_input
            .get("chars")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|chars| !chars.is_empty())
            .map(str::to_string);
    }
    if surface == ToolSurface::CodexFuzzyFileSearch {
        return extract_file_search_command(&normalized_tool_name, tool_input);
    }
    if surface != ToolSurface::CodexShell {
        return None;
    }
    for key in ["cmd", "command"] {
        if let Some(command) = tool_input.get(key).and_then(Value::as_str) {
            return Some(command.to_string());
        }
    }
    if let Some(command) = tool_input
        .get("args")
        .and_then(Value::as_array)
        .and_then(|values| string_array_command(values))
    {
        return Some(command);
    }
    if normalized_tool_name == "command_execution" {
        return tool_input
            .get("tool_input")
            .and_then(|value| value.get("command"))
            .and_then(Value::as_str)
            .map(str::to_string);
    }
    None
}

fn extract_file_search_command(tool_name: &str, tool_input: &Value) -> Option<String> {
    match tool_name {
        "grep" => {
            let pattern = payload_string(tool_input, "pattern")?;
            let mut command = vec!["rg".to_string()];
            if let Some(glob) = payload_string(tool_input, "glob") {
                command.push("--glob".to_string());
                command.push(shell_quote_arg(&glob));
            }
            command.push(shell_quote_arg(&pattern));
            if let Some(path) = payload_string(tool_input, "path") {
                command.push(shell_quote_arg(&path));
            }
            Some(command.join(" "))
        }
        "glob" => {
            let pattern = payload_string(tool_input, "pattern")?;
            let mut command = vec!["fd".to_string(), shell_quote_arg(&pattern)];
            if let Some(path) = payload_string(tool_input, "path") {
                command.push(shell_quote_arg(&path));
            }
            Some(command.join(" "))
        }
        _ => None,
    }
}

fn extract_paths_direct(tool_input: &Value) -> Vec<String> {
    let mut paths = path_values_for_keys(tool_input, PATH_SCALAR_KEYS);
    paths.extend(path_values_for_keys(tool_input, PATH_CONTAINER_KEYS));
    paths
}

fn path_values_for_keys(tool_input: &Value, keys: &[&str]) -> Vec<String> {
    keys.iter()
        .filter_map(|key| tool_input.get(*key))
        .flat_map(path_values)
        .collect()
}

fn path_values(value: &Value) -> Vec<String> {
    if let Some(path) = value.as_str() {
        return vec![normalize_path_value(path)];
    }
    if value.is_object() {
        return path_values_for_keys(value, PATH_VALUE_KEYS);
    }
    value
        .as_array()
        .into_iter()
        .flatten()
        .flat_map(path_values)
        .collect()
}

fn normalize_path_value(path: &str) -> String {
    let Some(uri_path) = path.strip_prefix("file://") else {
        return path.to_string();
    };
    if let Some(localhost_path) = uri_path.strip_prefix("localhost/") {
        return format!("/{localhost_path}");
    }
    uri_path.to_string()
}

fn tool_input_needs_action_scan(tool_name: &str, tool_input: &Value) -> bool {
    if is_codex_command_execution_tool_name(tool_name) {
        return true;
    }
    let Some(object) = tool_input.as_object() else {
        return false;
    };
    ACTION_SCAN_KEYS.iter().any(|key| object.contains_key(*key))
}

fn is_codex_command_execution_tool_name(tool_name: &str) -> bool {
    let leaf = tool_name.rsplit(['.', ':']).next().unwrap_or(tool_name);
    matches!(leaf, "command_execution" | "command-execution")
}

struct NestedToolAction {
    tool_name: String,
    input: Value,
}

fn nested_tool_actions(tool_input: &Value) -> Vec<NestedToolAction> {
    let mut nested = Vec::new();
    if let Some(action) = nested_function_action(tool_input) {
        nested.push(action);
    }
    for key in ["tool_uses", "toolUses", "tools", "tool_calls", "toolCalls"] {
        let Some(tool_uses) = tool_input.get(key).and_then(Value::as_array) else {
            continue;
        };
        for tool_use in tool_uses {
            if let Some(action) = nested_action_from_tool_use(tool_use) {
                nested.push(action);
            }
        }
    }
    nested
}

fn nested_action_from_tool_use(tool_use: &Value) -> Option<NestedToolAction> {
    if let Some(action) = nested_function_action(tool_use) {
        return Some(action);
    }
    let tool_name = payload_string(tool_use, "recipient_name")
        .or_else(|| payload_string(tool_use, "recipientName"))
        .or_else(|| payload_string(tool_use, "tool_name"))
        .or_else(|| payload_string(tool_use, "toolName"))
        .or_else(|| payload_string(tool_use, "name"))?;
    Some(NestedToolAction {
        tool_name,
        input: nested_input_value(tool_use),
    })
}

fn nested_function_action(value: &Value) -> Option<NestedToolAction> {
    let function = value.get("function")?;
    let tool_name = payload_string(function, "name")?;
    let input = function
        .get("arguments")
        .or_else(|| function.get("parameters"))
        .or_else(|| function.get("input"))
        .map(decoded_or_cloned)
        .unwrap_or(Value::Null);
    Some(NestedToolAction { tool_name, input })
}

fn nested_input_value(tool_use: &Value) -> Value {
    tool_use
        .get("parameters")
        .or_else(|| tool_use.get("tool_input"))
        .or_else(|| tool_use.get("toolInput"))
        .or_else(|| tool_use.get("input"))
        .or_else(|| tool_use.get("arguments"))
        .map(decoded_or_cloned)
        .unwrap_or(Value::Null)
}

fn decoded_or_cloned(value: &Value) -> Value {
    decoded_json_input(value).unwrap_or_else(|| value.clone())
}

fn decoded_json_input(value: &Value) -> Option<Value> {
    let text = value.as_str()?;
    serde_json::from_str::<Value>(text).ok()
}

fn string_array_command(values: &[Value]) -> Option<String> {
    let mut parts = Vec::new();
    for value in values {
        parts.push(shell_quote_arg(value.as_str()?));
    }
    if parts.is_empty() {
        None
    } else {
        Some(parts.join(" "))
    }
}

fn shell_quote_arg(value: &str) -> String {
    if value.chars().any(char::is_whitespace) {
        format!("'{}'", value.replace('\'', "'\"'\"'"))
    } else {
        value.to_string()
    }
}

fn push_unique_path(paths: &mut Vec<String>, path: String) {
    if !paths.iter().any(|existing| existing == &path) {
        paths.push(path);
    }
}
