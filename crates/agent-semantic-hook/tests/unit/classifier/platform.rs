// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use super::registry;
use agent_semantic_hook::classify_hook;
use agent_semantic_hook::render_platform_response;
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
                "command": "asp search playbook --language typescript --rg 'ParserService cacheDir'"
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
    assert_eq!(context, decision.message);
}

#[test]
fn permission_request_uses_codex_decision_object_contract() {
    let decision = classify_hook(
        &registry(),
        "codex",
        "permission-request",
        &json!({
            "hook_event_name": "PermissionRequest",
            "tool_name": "Bash",
            "tool_input": {
                "command": "asp search playbook --language typescript --rg 'ParserService cacheDir'"
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
        response["hookSpecificOutput"]["decision"]["behavior"],
        "allow"
    );
    assert!(response["hookSpecificOutput"]["permissionDecision"].is_null());
}

#[test]
fn permission_request_deny_uses_codex_decision_object_with_agent_message() {
    let mut decision = classify_hook(
        &registry(),
        "codex",
        "permission-request",
        &json!({
            "hook_event_name": "PermissionRequest",
            "tool_name": "Bash",
            "tool_input": { "command": "rg -n TODO ." }
        }),
    );
    decision.decision = agent_semantic_hook::DecisionKind::Deny;
    decision.message =
        "编辑模式必须在主线程完成，当前的 asp_explorer 不允许执行 edit action。".to_string();

    let response = render_platform_response(&decision).unwrap();
    assert_eq!(decision.decision, agent_semantic_hook::DecisionKind::Deny);
    assert_eq!(
        response["hookSpecificOutput"]["decision"]["behavior"],
        "deny"
    );
    assert_eq!(
        response["hookSpecificOutput"]["decision"]["message"],
        response["systemMessage"]
    );
    assert!(
        response["hookSpecificOutput"]["decision"]["message"]
            .as_str()
            .is_some_and(|message| !message.trim().is_empty())
    );
    assert!(response["hookSpecificOutput"]["permissionDecision"].is_null());
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
        "ASP Search playbook routing is active for this prompt",
        "`asp search playbook '<scheme-expression>'`",
        "`(producers (language ...) (documents ...))`",
        "begin V1 composition with `(intersect (rg ...) (tantivy ...))`",
        "Preserve every native argv token as a Scheme string",
        "one admitted immutable workspace generation",
        "without prescribing the Agent's next action",
        "one `asp query playbook` request",
        "`--selector <exact-selector>`",
        "Query has no implicit Search state",
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
        "ASP Search playbook routing is active for this locator question",
        "`asp search playbook '<scheme-expression>'`",
        "`(producers (language ...) (documents ...))`",
        "with `graph` leaves last",
        "Preserve every native argv token as a Scheme string",
        "one admitted immutable workspace generation",
        "without prescribing the Agent's next action",
        "one `asp query playbook` request",
        "`--selector <exact-selector>`",
        "Query has no implicit Search state",
    ] {
        assert!(
            context.contains(expected),
            "missing `{expected}` in {context}"
        );
    }
    assert!(!context.contains("--code"), "{context}");
    assert!(!context.contains("--names-only"), "{context}");
}
