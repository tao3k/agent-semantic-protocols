//! Renders agent-facing Hook decisions from compiled, declarative rule fields.

use super::CompiledHookRule;
use crate::agent_dispatch_message::{AgentDispatchMessageFields, render_choice_plane_instruction};

pub(super) fn render(rule: &CompiledHookRule) -> String {
    let fallback = format!(
        "client hook config rule `{}` matched this tool use",
        rule.id
    );
    let Some(template) = rule.message.as_deref() else {
        return fallback;
    };
    let execution_lane = rule.fields.get("executionLane").map_or("", String::as_str);
    let target_agent_name = rule
        .dispatch
        .as_ref()
        .map_or("", |dispatch| dispatch.resident_codex_agent_name.as_str());
    let target_agent_role = rule
        .dispatch
        .as_ref()
        .map_or("", |dispatch| dispatch.resident_role.as_str());
    let target_agent_kind = rule
        .dispatch
        .as_ref()
        .map_or("Subagent", |dispatch| dispatch.resident_agent_kind.as_str());
    let target_agent_display_role = rule
        .dispatch
        .as_ref()
        .map_or("", |dispatch| dispatch.resident_display_role.as_str());
    let target_agent_description = rule
        .dispatch
        .as_ref()
        .map_or("", |dispatch| dispatch.resident_description.as_str());
    let agent_session_instruction = render_choice_plane_instruction(AgentDispatchMessageFields {
        agent_kind: target_agent_kind,
        call_target: target_agent_name,
        role: target_agent_display_role,
        description: target_agent_description,
    });
    agent_semantic_config::render_hook_client_message_template(
        template,
        &[
            ("executionLane", execution_lane),
            ("targetAgentName", target_agent_name),
            ("targetAgentRole", target_agent_role),
            ("targetAgentKind", target_agent_kind),
            ("targetAgentDisplayRole", target_agent_display_role),
            ("targetAgentDescription", target_agent_description),
            ("agentWindowCommand", "asp session --agents choice-plane"),
            ("agentSessionInstruction", &agent_session_instruction),
        ],
    )
}
