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
    let target_agent_role = rule
        .dispatch
        .as_ref()
        .map_or("", |dispatch| dispatch.target_role.as_str());
    let receipt_kind = rule
        .dispatch
        .as_ref()
        .map_or("", |dispatch| dispatch.receipt_kind.as_str());
    let agent_dispatch_message = render_choice_plane_instruction(AgentDispatchMessageFields {
        role: target_agent_role,
        receipt_kind,
    });
    agent_semantic_config::render_hook_client_message_template(
        template,
        &[
            ("executionLane", execution_lane),
            ("targetAgentRole", target_agent_role),
            ("receiptKind", receipt_kind),
            ("agentWindowCommand", "asp session --agents choice-plane"),
            ("agentDispatchMessage", &agent_dispatch_message),
        ],
    )
}
