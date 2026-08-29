//! Normalizes platform tool payloads into hook classifier actions.

use std::borrow::Cow;

use serde_json::Value;

use crate::command::{apply_patch_source_paths, semantic_shell_tokens};
use crate::protocol::DecisionSubject;

#[path = "tool_action_exec/functions_exec.rs"]
mod functions_exec;
#[path = "shell_action_segments.rs"]
mod shell_segments;

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
pub(crate) use crate::action_ir::{
    AgentAction, AgentActionKind, AgentActionSubject, AgentActionSubjectKind,
    FilesystemPermissionFact, FilesystemPermissionKind, FilesystemPermissionSource,
    HostInvocationFact, HostInvocationKind, SemanticCapability, SemanticCapabilityEvidence,
    action_kind_matches,
};

pub(crate) fn subject_kind_matches(
    candidate: AgentActionSubjectKind,
    configured: agent_semantic_config::HookClientActionSubjectKind,
) -> bool {
    use agent_semantic_config::HookClientActionSubjectKind as Configured;

    matches!(
        (candidate, configured),
        (
            AgentActionSubjectKind::RegisteredLanguageSource,
            Configured::RegisteredLanguageSource
        ) | (
            AgentActionSubjectKind::RegisteredLanguageSourcePattern,
            Configured::RegisteredLanguageSourcePattern
        ) | (AgentActionSubjectKind::Directory, Configured::Directory)
            | (
                AgentActionSubjectKind::StructuralSelector,
                Configured::StructuralSelector
            )
            | (AgentActionSubjectKind::Other, Configured::Other)
    )
}

#[derive(Clone, Debug)]
pub(crate) struct ToolAction {
    pub(crate) tool_name: String,
    pub(crate) host_payload: Value,
    pub(crate) invocation_source: Option<String>,
    /// Immutable Host action supplied by the exact plugin matcher entry.
    /// Shell parsing may add semantic capabilities but never rewrites this fact.
    pub(crate) host_action: HostInvocationKind,
    pub(crate) surface: ToolSurface,
    pub(crate) operation: OperationIntent,
    pub(crate) command: Option<String>,
    pub(crate) command_tokens: Option<Vec<String>>,
    /// Original Host shell envelope. Compound-command stage projection must
    /// retain this invocation-level fact so process-environment transfer is
    /// not lost when individual stages are classified.
    pub(crate) shell_envelope_command: Option<String>,
    pub(crate) paths: Vec<String>,
    pub(crate) has_declared_filesystem_access: bool,
}

impl ToolAction {
    pub(crate) fn normalized_direct_policy_action(path: String) -> Self {
        Self {
            tool_name: "Read".to_owned(),
            host_payload: serde_json::json!({ "path": path.as_str() }),
            invocation_source: None,
            host_action: HostInvocationKind::Read,
            surface: ToolSurface::CodexDirectRead,
            operation: OperationIntent::DirectRead,
            command: None,
            command_tokens: None,
            shell_envelope_command: None,
            paths: vec![path],
            // Synthetic policy action; no parser-owned shell projection here.
            has_declared_filesystem_access: false,
        }
    }

    pub(crate) fn normalized_shell_policy_action(command: String, path: String) -> Self {
        let command_tokens = semantic_shell_tokens(&command);
        Self {
            tool_name: "Bash".to_owned(),
            host_payload: serde_json::json!({ "command": command.as_str(), "path": path.as_str() }),
            invocation_source: None,
            host_action: HostInvocationKind::Execute,
            surface: ToolSurface::CodexShell,
            operation: OperationIntent::ShellCommand,
            command: Some(command),
            command_tokens: Some(command_tokens),
            shell_envelope_command: None,
            paths: vec![path],
            // Synthetic policy action; no parser-owned shell projection here.
            has_declared_filesystem_access: false,
        }
    }

    pub(crate) fn normalized_shell_command_action(command: String, tool_name: String) -> Self {
        let command_tokens = semantic_shell_tokens(&command);
        let host_action = if tool_name == "Bash" {
            HostInvocationKind::Execute
        } else {
            HostInvocationKind::Unknown
        };
        Self {
            tool_name,
            host_payload: serde_json::json!({ "command": command.as_str() }),
            invocation_source: None,
            host_action,
            surface: ToolSurface::CodexShell,
            operation: OperationIntent::ShellCommand,
            shell_envelope_command: Some(command.clone()),
            command: Some(command),
            command_tokens: Some(command_tokens),
            paths: Vec::new(),
            // Synthetic envelope action; split_shell_command owns parsed facts.
            has_declared_filesystem_access: false,
        }
    }

    pub(crate) fn semantic_command_text(&self) -> Option<&str> {
        self.command.as_deref()
    }

    pub(crate) fn derive_agent_action(&self) -> AgentAction {
        self.derive_agent_action_with_shell_facts().0
    }

    pub(crate) fn derive_agent_action_with_shell_facts(
        &self,
    ) -> (
        AgentAction,
        Vec<agent_semantic_shell_parser::CommandStage>,
        Vec<agent_semantic_shell_parser::ShellBehaviorFact>,
    ) {
        let mut action = AgentAction {
            host: HostInvocationFact {
                action: self.host_action,
                tool_name: self.tool_name.clone(),
                surface: self.surface.as_str().to_owned(),
                payload: self.host_payload.clone(),
                invocation_source: self.invocation_source.clone(),
            },
            filesystem_permissions: Vec::new(),
            capabilities: Vec::new(),
            subjects: Vec::new(),
        };
        let semantic_action = match self.host_action {
            HostInvocationKind::Read => AgentActionKind::Read,
            HostInvocationKind::Edit => AgentActionKind::Edit,
            HostInvocationKind::Execute => AgentActionKind::Execute,
            HostInvocationKind::Mcp => AgentActionKind::Mcp,
            HostInvocationKind::SpawnAgent => AgentActionKind::SpawnAgent,
            HostInvocationKind::Unknown => AgentActionKind::Unknown,
        };
        let permission = match semantic_action {
            AgentActionKind::Read => Some(FilesystemPermissionKind::Read),
            AgentActionKind::Edit => Some(FilesystemPermissionKind::Write),
            _ => None,
        };
        if let Some(permission) = permission {
            if self.paths.is_empty() {
                action.add_capability(SemanticCapability {
                    action: semantic_action,
                    evidence: SemanticCapabilityEvidence::HostMatcher,
                });
            } else {
                for path in &self.paths {
                    action.add_filesystem_permission(FilesystemPermissionFact::new(
                        permission,
                        FilesystemPermissionSource::HostMatcher,
                        Some(path.clone()),
                    ));
                }
            }
        } else if semantic_action != AgentActionKind::Unknown {
            action.add_capability(SemanticCapability {
                action: semantic_action,
                evidence: SemanticCapabilityEvidence::HostMatcher,
            });
        }
        let command_stages = if matches!(
            self.surface,
            ToolSurface::CodexShell | ToolSurface::CodexStdinContinuation
        ) {
            self.semantic_command_text()
                .and_then(|command| {
                    agent_semantic_shell_parser::parse_bash_command_candidates(command).ok()
                })
                .into_iter()
                .flatten()
                .collect::<Vec<_>>()
        } else {
            Vec::new()
        };
        let behavior_facts = command_stages
            .iter()
            .flat_map(agent_semantic_shell_parser::command_stage_behavior_facts)
            .collect::<Vec<_>>();
        for fact in &behavior_facts {
            let permission = match fact.access {
                agent_semantic_shell_parser::ShellAccessKind::Read => {
                    crate::action_ir::FilesystemPermissionKind::Read
                }
                agent_semantic_shell_parser::ShellAccessKind::Write => {
                    crate::action_ir::FilesystemPermissionKind::Write
                }
            };
            action.add_filesystem_permission(FilesystemPermissionFact::new(
                permission,
                FilesystemPermissionSource::ShellRedirection,
                fact.subject.clone(),
            ));
        }
        (action, command_stages, behavior_facts)
    }

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
        _paths: &[String],
    ) -> Self {
        match surface {
            ToolSurface::CodexApplyPatch => Self::ApplyPatch,
            ToolSurface::CodexDirectRead | ToolSurface::CodexMcpRead => Self::DirectRead,
            ToolSurface::CodexDirectoryRead => Self::DirectoryRead,
            ToolSurface::CodexFuzzyFileSearch => Self::FileSearch,
            ToolSurface::CodexNestedTools => Self::NestedTools,
            ToolSurface::CodexShell if command.is_some() => Self::ShellCommand,
            ToolSurface::CodexStdinContinuation if command.is_some() => Self::StdinContinuation,
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

/// Returns parser-owned paths when the payload contains a direct source-read action.
pub fn direct_source_read_paths(tool_name: &str, tool_input: &Value) -> Option<Vec<String>> {
    collect_tool_actions(tool_name, tool_input)
        .into_iter()
        .find(|action| action.operation == OperationIntent::DirectRead)
        .map(|action| action.paths)
}

/// Parser-owned command-envelope projections used by generated wrapped-command
/// coverage. Host Read spellings are deliberately absent: Codex exposes shell
/// execution as `Bash`, and the parser owns its source-access projection.
pub(crate) fn shell_host_envelopes(command: &str) -> Vec<(String, Value)> {
    let encoded = serde_json::to_string(command).expect("command JSON encoding");
    vec![
        ("Bash".to_owned(), serde_json::json!({"command": command})),
        (
            "functions.exec_command".to_owned(),
            serde_json::json!({"cmd": command}),
        ),
        (
            "exec_command".to_owned(),
            serde_json::json!({"args": semantic_shell_tokens(command)}),
        ),
        (
            "functions.exec".to_owned(),
            Value::String(format!(
                "const receipt = await tools.exec_command({{cmd: {encoded}}}); text(receipt);"
            )),
        ),
    ]
}

/// Returns whether a Codex tool envelope can reach any ASP policy-bearing
/// action without loading configuration or contacting the Runtime Server.
///
/// `None` means the host envelope omitted its typed tool identity and must be
/// handled fail-closed by the full evaluator. `Some(false)` is an authoritative
/// no-I/O passthrough for unrelated actions such as planning or UI tools.
pub fn codex_tool_event_requires_policy_evaluation(payload: &Value) -> Option<bool> {
    let tool_name = payload
        .get("tool_name")
        .or_else(|| payload.get("toolName"))?
        .as_str()?;
    let tool_input = payload
        .get("tool_input")
        .or_else(|| payload.get("toolInput"))
        .unwrap_or(&Value::Null);
    let actions = collect_tool_actions(tool_name, tool_input);
    if actions
        .iter()
        .any(|action| action.operation != OperationIntent::Unknown)
    {
        return Some(true);
    }
    // A dynamic or unsupported JavaScript envelope is still policy-bearing:
    // the local evaluator must fail closed instead of treating it as an
    // unrelated host action merely because no literal command was projected.
    if tool_name == "functions.exec"
        && functions_exec::functions_exec_source(tool_input)
            .is_some_and(|code| code.contains("tools.exec_command"))
    {
        return Some(true);
    }
    Some(false)
}

/// Collects source intents from direct, shell, nested, and Codex actions.
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

        // Nested native tool envelopes must re-enter canonical normalization;
        // otherwise registered source extensions disappear before matching.
        if direct_action && let Some(nested) = nested_action_from_tool_use(value) {
            for action in collect_tool_actions(&nested.tool_name, &nested.input) {
                push_unique_action(actions, action);
            }
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
                ToolSurface::CodexFuzzyFileSearch,
                OperationIntent::FileSearch,
                command,
            ),
            "unknown" => (
                ToolSurface::CodexShell,
                OperationIntent::ShellCommand,
                command,
            ),
            _ => return None,
        };

        if command.is_none()
            && paths.is_empty()
            && !matches!(operation, OperationIntent::FileSearch)
        {
            return None;
        }
        let command_tokens = command.as_deref().map(semantic_shell_tokens);

        Some(ToolAction {
            tool_name: format!("{tool_name}.command_action.{action_type}"),
            host_payload: value.clone(),
            invocation_source: None,
            host_action: HostInvocationKind::Unknown,
            surface,
            operation,
            command,
            command_tokens,
            shell_envelope_command: None,
            paths,
            has_declared_filesystem_access: false,
        })
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
    let opaque_control_payload = command
        .as_deref()
        .is_some_and(is_hook_break_glass_mint_control_command);
    let scans_nested_actions = tool_input_needs_action_scan(tool_name, tool_input);
    if surface == ToolSurface::CodexShell
        && !opaque_control_payload
        && let Some(command) = command.as_deref()
        && let Some(compound_actions) =
            shell_segments::split_shell_command(tool_name, command, tool_input, None)
    {
        let mut actions = Vec::new();
        if scans_nested_actions {
            actions.extend(codex_command_actions(tool_name, tool_input));
            for nested in nested_tool_actions(tool_name, tool_input) {
                actions.extend(collect_tool_actions(&nested.tool_name, &nested.input));
            }
        }
        for action in compound_actions {
            push_unique_action(&mut actions, action);
        }
        return actions;
    }
    let command_tokens = command.as_deref().map(semantic_shell_tokens);
    let mut paths = if scans_nested_actions && surface == ToolSurface::Unknown {
        Vec::new()
    } else if surface == ToolSurface::CodexApplyPatch {
        extract_apply_patch_paths_direct(tool_input)
    } else {
        extract_paths_direct(tool_input)
    };
    if let Some(command) = command.as_deref() {
        let patch_paths = apply_patch_source_paths(tool_name, command);
        for path in patch_paths {
            if !paths.iter().any(|existing| existing == &path) {
                paths.push(path);
            }
        }
        if surface != ToolSurface::CodexApplyPatch && !opaque_control_payload {
            let command_paths = agent_semantic_shell_parser::command_source_paths(
                command,
                command_tokens.as_deref().unwrap_or_default(),
            );
            for path in command_paths {
                if !paths.iter().any(|existing| existing == &path) {
                    paths.push(path);
                }
            }
        }
    }
    let operation = OperationIntent::from_action(surface, command.as_deref(), &paths);
    let has_declared_filesystem_access = command
        .as_deref()
        .and_then(|text| agent_semantic_shell_parser::parse_bash_command_candidates(text).ok())
        .into_iter()
        .flatten()
        .flat_map(|stage| agent_semantic_shell_parser::command_stage_behavior_facts(&stage))
        .any(|fact| fact.subject.is_some());
    let envelope_action = ToolAction {
        tool_name: tool_name.to_string(),
        host_payload: tool_input.clone(),
        invocation_source: None,
        host_action: HostInvocationKind::Unknown,
        surface,
        operation,
        command,
        command_tokens,
        shell_envelope_command: None,
        paths,
        has_declared_filesystem_access,
    };
    let mut actions = Vec::new();
    if scans_nested_actions {
        actions.extend(codex_command_actions(tool_name, tool_input));
        for nested in nested_tool_actions(tool_name, tool_input) {
            actions.extend(collect_tool_actions(&nested.tool_name, &nested.input));
        }
    }
    push_unique_action(&mut actions, envelope_action);
    actions
}

fn is_hook_break_glass_mint_control_command(command: &str) -> bool {
    semantic_shell_tokens(command)
        .get(..4)
        .is_some_and(|prefix| prefix == ["asp", "hook", "break-glass", "mint"])
}

/// Projects workspace mutation paths directly from the canonical tool-action
/// normalization. This projection is independent of policy matching so
/// post-tool durability cannot disappear when no allow/deny rule applies.
pub fn workspace_mutation_paths(tool_name: &str, tool_input: &Value) -> Vec<String> {
    let mut paths = collect_tool_actions(tool_name, tool_input)
        .into_iter()
        .filter(|action| action.operation == OperationIntent::ApplyPatch)
        .flat_map(|action| action.paths)
        .filter(|path| !path.trim().is_empty())
        .collect::<Vec<_>>();
    paths.sort();
    paths.dedup();
    paths
}

#[cfg(test)]
#[path = "../tests/unit/tool_action_functions_exec.rs"]
mod functions_exec_tests;

#[cfg(test)]
#[path = "../tests/unit/tool_action_workspace_mutation.rs"]
mod workspace_mutation_tests;

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
        return extract_apply_patch_text_direct(tool_input).map(str::to_string);
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
        return None;
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

fn tool_input_needs_action_scan(tool_name: &str, tool_input: &Value) -> bool {
    if is_codex_command_execution_tool_name(tool_name) {
        return true;
    }
    if tool_name == "functions.exec" && functions_exec::functions_exec_source(tool_input).is_some()
    {
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

fn nested_tool_actions(tool_name: &str, tool_input: &Value) -> Vec<NestedToolAction> {
    let mut nested = Vec::new();
    if let Some(action) = nested_function_action(tool_input) {
        nested.push(action);
    }
    nested.extend(functions_exec::nested_code_actions(tool_name, tool_input));
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
        parts.push(render_shell_token(value.as_str()?));
    }
    if parts.is_empty() {
        None
    } else {
        Some(parts.join(" "))
    }
}

fn render_shell_token(value: &str) -> String {
    if value.chars().any(char::is_whitespace) {
        format!("'{}'", value.replace('\'', "'\"'\"'"))
    } else {
        value.to_string()
    }
}

#[path = "tool_action_paths.rs"]
mod paths;

use paths::extract_apply_patch_text_direct;
use paths::{
    extract_apply_patch_paths_direct, extract_paths_direct, path_values, push_unique_path,
};
