use super::registry;
use agent_semantic_hook::{classify_hook, render_platform_response};
use serde_json::json;

#[test]
fn permission_request_allow_renders_explicit_allow_for_claude() {
    let decision = classify_hook(
        &registry(),
        "claude",
        "permission-request",
        &json!({
            "hook_event_name": "PermissionRequest",
            "tool_name": "Bash",
            "tool_input": {
                "command": "asp typescript search prime --workspace . --view seeds"
            }
        }),
    );

    let response = render_platform_response(&decision).unwrap();
    assert_eq!(decision.decision, agent_semantic_hook::DecisionKind::Allow);
    assert_eq!(
        response["hookSpecificOutput"]["hookEventName"],
        "PermissionRequest"
    );
    assert_eq!(
        response["hookSpecificOutput"]["permissionDecision"],
        "allow"
    );
    let context = response["hookSpecificOutput"]["additionalContext"]
        .as_str()
        .expect("decision context");
    assert!(context.contains("\"decision\":\"allow\""), "{context}");
}

#[test]
fn user_prompt_submit_allow_adds_search_first_context_for_claude() {
    let decision = classify_hook(
        &registry(),
        "claude",
        "user-prompt",
        &json!({
            "hook_event_name": "UserPromptSubmit",
            "prompt": "How is AsyncRead implemented?"
        }),
    );

    let response = render_platform_response(&decision).unwrap();
    let context = response["hookSpecificOutput"]["additionalContext"]
        .as_str()
        .expect("user prompt additional context");
    assert_eq!(
        response["hookSpecificOutput"]["hookEventName"],
        "UserPromptSubmit"
    );
    assert!(response["hookSpecificOutput"]["permissionDecision"].is_null());
    for expected in [
        "ASP evidence-state search routing",
        "Search is not a mandatory pipeline",
        "Choose the narrowest ASP route",
        "Do not answer from prime alone",
        "ASP facades are language IDs",
        "query --selector <exact-selector> --workspace . --projection source",
        "return one compact `[asp-search-subagent]` graph-route receipt",
        "never source bodies or line-range selectors",
        "Do not use direct source reads as the first step",
    ] {
        assert!(
            context.contains(expected),
            "missing `{expected}` in {context}"
        );
    }
}

#[test]
fn user_prompt_submit_locator_questions_do_not_push_code_reads() {
    let decision = classify_hook(
        &registry(),
        "claude",
        "user-prompt",
        &json!({
            "hook_event_name": "UserPromptSubmit",
            "prompt": "Where is AsyncRead implemented before selecting files to edit?"
        }),
    );

    let response = render_platform_response(&decision).unwrap();
    let context = response["hookSpecificOutput"]["additionalContext"]
        .as_str()
        .expect("user prompt additional context");
    for expected in [
        "locator/frontier question",
        "answer where to look before editing",
        "Do not answer from prime alone",
        "ASP facades are language IDs",
        "query --selector <exact-selector> --workspace . --projection source",
        "compact `[asp-search-subagent]` graph-route receipt",
    ] {
        assert!(
            context.contains(expected),
            "missing `{expected}` in {context}"
        );
    }
    assert!(!context.contains("--code"), "{context}");
    assert!(!context.contains("--names-only"), "{context}");
}
