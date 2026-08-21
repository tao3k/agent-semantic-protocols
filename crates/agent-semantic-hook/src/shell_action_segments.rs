//! Parser-owned shell-stage projection for declarative Hook classification.

use crate::tool_action::{OperationIntent, ToolAction, ToolSurface};

/// Parses a compound shell command once and projects one declarative Hook
/// action per executable stage. Simple commands stay on the envelope fast path.
pub(super) fn split_shell_command(tool_name: &str, command: &str) -> Option<Vec<ToolAction>> {
    let Ok(stages) = agent_semantic_command_match::parse_bash_command_candidates(command) else {
        return None;
    };
    let stages = stages
        .into_iter()
        .filter(|stage| !stage.is_separator())
        .collect::<Vec<_>>();
    if stages.len() <= 1 {
        return None;
    }
    Some(
        stages
            .into_iter()
            .enumerate()
            .map(|(index, stage)| {
                let command_tokens = stage.words().to_vec();
                let command = agent_semantic_command_match::render_bash_command_stage(&stage);
                let mut paths = agent_semantic_command_match::command_stage_source_paths(&stage);
                for path in crate::command::apply_patch_source_paths(tool_name, &command) {
                    if !paths.contains(&path) {
                        paths.push(path);
                    }
                }
                ToolAction {
                    tool_name: tool_name.to_string(),
                    surface: ToolSurface::CodexShell,
                    operation: OperationIntent::ShellCommand,
                    command: Some(command),
                    command_tokens: Some(command_tokens),
                    leading_shell_stage: index == 0,
                    paths,
                }
            })
            .collect(),
    )
}
