//! Parser-owned shell-stage projection for declarative Hook classification.

use crate::tool_action::{OperationIntent, ToolAction, ToolSurface};

/// Parses a compound shell command once and projects one declarative Hook
/// action per executable stage. Simple commands stay on the envelope fast path.
pub(super) fn split_shell_command(
    tool_name: &str,
    command: &str,
    host_payload: &serde_json::Value,
    invocation_source: Option<&str>,
) -> Option<Vec<ToolAction>> {
    let Ok(stages) = agent_semantic_shell_parser::parse_bash_command_candidates(command) else {
        return None;
    };
    let stages = stages
        .into_iter()
        .filter(|stage| !stage.is_separator())
        .collect::<Vec<_>>();
    if stages.len() <= 1 {
        return None;
    }
    let has_declared_filesystem_access = stages
        .iter()
        .flat_map(agent_semantic_shell_parser::command_stage_behavior_facts)
        .any(|fact| fact.subject.is_some());
    let shell_envelope_command = command.to_owned();
    Some(
        stages
            .into_iter()
            .map(|stage| {
                let command_tokens = stage.words().to_vec();
                let command = agent_semantic_shell_parser::render_bash_command_stage(&stage);
                let mut paths = agent_semantic_shell_parser::command_stage_source_paths(&stage);
                for path in crate::command::apply_patch_source_paths(tool_name, &command) {
                    if !paths.contains(&path) {
                        paths.push(path);
                    }
                }
                ToolAction {
                    tool_name: tool_name.to_string(),
                    host_payload: host_payload.clone(),
                    invocation_source: invocation_source.map(str::to_owned),
                    host_action: crate::action_ir::HostInvocationKind::Unknown,
                    surface: ToolSurface::CodexShell,
                    operation: OperationIntent::ShellCommand,
                    command: Some(command),
                    command_tokens: Some(command_tokens),
                    shell_envelope_command: Some(shell_envelope_command.clone()),
                    paths,
                    has_declared_filesystem_access,
                }
            })
            .collect(),
    )
}
