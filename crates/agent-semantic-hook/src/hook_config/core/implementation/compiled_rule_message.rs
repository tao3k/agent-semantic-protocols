//! Renders agent-facing Hook decisions from compiled, declarative rule fields.

use super::CompiledHookRule;
pub(super) fn render(rule: &CompiledHookRule, _platform: &str) -> String {
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
    agent_semantic_config::render_hook_client_message_template(
        template,
        &[
            ("executionLane", execution_lane),
            ("targetAgent", target_agent),
            ("receiptKind", receipt_kind),
            // The parent session id is a Host payload fact and is unavailable
            // while compiling Config. The classifier appends the complete
            // Collaboration instruction after it receives that payload.
            ("agentDispatchMessage", ""),
        ],
    )
}
