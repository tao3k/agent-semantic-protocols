//! Renders agent-facing Hook decisions from compiled, declarative rule fields.

use super::CompiledHookRule;
use crate::agent_dispatch_message::{AgentDispatchMessageFields, render_choice_plane_instruction};

pub(super) fn render(rule: &CompiledHookRule, platform: &str) -> String {
    let fallback = format!(
        "client hook config rule `{}` matched this tool use",
        rule.id
    );
    let Some(template) = rule.message.as_deref() else {
        return fallback;
    };
    let execution_lane = rule.fields.get("executionLane").map_or("", String::as_str);
    let target_agent = rule
        .dispatch
        .as_ref()
        .map_or("", |dispatch| dispatch.target_agent.as_str());
    let receipt_kind = rule
        .dispatch
        .as_ref()
        .map_or("", |dispatch| dispatch.receipt_kind.as_str());
    let target_agent_symbol = rule.dispatch.as_ref().map(|dispatch| {
        dispatch
            .calling
            .symbol(platform, dispatch.target_agent.as_str())
    });
    let agent_dispatch_message = render_choice_plane_instruction(AgentDispatchMessageFields {
        agent: target_agent,
        symbol: target_agent_symbol.as_deref(),
        receipt_kind,
    });
    agent_semantic_config::render_hook_client_message_template(
        template,
        &[
            ("executionLane", execution_lane),
            ("targetAgent", target_agent),
            ("receiptKind", receipt_kind),
            ("agentWindowCommand", "asp session --agents choice-plane"),
            ("agentDispatchMessage", &agent_dispatch_message),
        ],
    )
}
