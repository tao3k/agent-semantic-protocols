use semantic_agent_hook::{classify_hook, render_platform_response};
use serde_json::json;

use super::registry;

#[test]
fn platform_response_wraps_denied_decision_for_codex_hooks() {
    let decision = classify_hook(
        &registry(),
        "codex",
        "pre-tool",
        &json!({
            "tool_name": "Read",
            "tool_input": {"path": "src/cli/agent-hooks.ts"}
        }),
    );

    let response = render_platform_response(&decision).unwrap();

    assert_eq!(
        response["hookSpecificOutput"]["hookEventName"],
        "PreToolUse"
    );
    assert_eq!(response["hookSpecificOutput"]["permissionDecision"], "deny");
    assert_eq!(response["agentHookDecision"]["decision"], "deny");
    assert_eq!(
        response["agentHookDecision"]["reasonKind"],
        "direct-source-read"
    );
}
