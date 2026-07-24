use serde_json::json;

use super::{claude_fixture, install_claude_hooks, run_claude_pre_tool_decision};

#[test]
fn claude_pre_tool_denies_source_directory_enumeration() {
    let root = claude_fixture();
    install_claude_hooks(root.as_path());
    let decision = run_claude_pre_tool_decision(
        root.as_path(),
        json!({"session_id":"session-claude-list-files","transcript_path":root.as_path().join("session.jsonl"),"cwd":root.as_path(),"hook_event_name":"PreToolUse","tool_use_id":"toolu_list_files","tool_name":"Bash","tool_input":{"command":"ls src","commandActions":[{"type":"listFiles","command":"ls src","path":"src"}]}}),
        &["--emit", "decision"],
    );
    assert_eq!(decision["platform"], "claude");
    assert_eq!(decision["decision"], "deny");
    assert_eq!(decision["reasonKind"], "source-directory-enumeration");
    assert_eq!(decision["routes"][0]["kind"], "ingest");
    assert_eq!(decision["subject"]["command"], "ls src");
}

#[test]
fn claude_platform_response_uses_hook_specific_permission_decision() {
    let root = claude_fixture();
    install_claude_hooks(root.as_path());
    let response = run_claude_pre_tool_decision(
        root.as_path(),
        json!({"session_id":"session-claude-read","transcript_path":root.as_path().join("session.jsonl"),"cwd":root.as_path(),"hook_event_name":"PreToolUse","tool_use_id":"toolu_read","tool_name":"Read","tool_input":{"file_path":root.as_path().join("src/lib.rs")}}),
        &[],
    );
    assert_eq!(
        response["hookSpecificOutput"]["hookEventName"],
        "PreToolUse"
    );
    assert_eq!(response["hookSpecificOutput"]["permissionDecision"], "deny");
    let reason = response["hookSpecificOutput"]["permissionDecisionReason"]
        .as_str()
        .expect("permission reason");
    assert!(reason.contains("ASP denied"), "{reason}");
    assert!(reason.contains("direct-source-read"), "{reason}");
    assert!(reason.contains("parser-owned route"), "{reason}");
    assert!(reason.contains("asp rust search owner"), "{reason}");
    assert!(!reason.contains("spawn_agent"), "{reason}");
    assert!(!reason.contains("asp_explorer"), "{reason}");
    assert!(response.get("agentHookDecision").is_none());
}

#[test]
fn claude_platform_response_compacts_repeated_denied_source_lane() {
    let root = claude_fixture();
    install_claude_hooks(root.as_path());
    let payload = |tool_use_id: &str| json!({"session_id":"session-claude-repeated-read","transcript_path":root.as_path().join("session.jsonl"),"cwd":root.as_path(),"hook_event_name":"PreToolUse","tool_use_id":tool_use_id,"tool_name":"Read","tool_input":{"file_path":root.as_path().join("src/lib.rs")}});
    let first = run_claude_pre_tool_decision(root.as_path(), payload("toolu_read_1"), &[]);
    let second = run_claude_pre_tool_decision(root.as_path(), payload("toolu_read_2"), &[]);
    assert_eq!(first["hookSpecificOutput"]["permissionDecision"], "deny");
    let reason = second["hookSpecificOutput"]["permissionDecisionReason"]
        .as_str()
        .expect("permission reason");
    assert!(
        reason.starts_with("ASP denied source access again (`direct-source-read`)"),
        "{reason}"
    );
    assert!(
        reason.contains("resident-child interactive loop")
            || reason.contains("interactive loop")
            || reason.contains("Use asp-explore"),
        "{reason}"
    );
    assert!(!reason.contains("## Agent Flow"));
    let context = second["hookSpecificOutput"]["additionalContext"]
        .as_str()
        .expect("decision context");
    assert!(context.contains("\"denyReplay\":\"repeated\""));
    assert!(
        context.contains("\"requiredAction\":\"send-to-asp-explore\"")
            || context.contains("\"requiredAction\":\"enter-asp-explore-choice-pane\"")
            || context
                .contains("\"requiredAction\":\"inspect-existing-child-and-recover-validation\"")
    );
    assert!(
        context.contains("\"nextAction\":\"run-asp-command-in-registered-asp-explore-child\"")
            || context.contains("\"nextAction\":\"choose-one-bootstrap-pane-option\"")
            || context
                .contains("\"nextAction\":\"ask-existing-child-to-switch-model-and-revalidate\"")
    );
    assert!(context.contains("\"forbiddenUntilResolved\":\"raw-source-fallback\""));
}

#[test]
fn claude_platform_response_compacts_cross_action_source_access_lane() {
    let root = claude_fixture();
    install_claude_hooks(root.as_path());
    let transcript_path = root.as_path().join("session.jsonl");
    let first = run_claude_pre_tool_decision(
        root.as_path(),
        json!({"session_id":"session-claude-cross-action-source-access","transcript_path":transcript_path,"cwd":root.as_path(),"hook_event_name":"PreToolUse","tool_use_id":"toolu_bash_raw_search","tool_name":"Bash","tool_input":{"command":"rg -n --glob '*.rs' demo src"}}),
        &[],
    );
    let second = run_claude_pre_tool_decision(
        root.as_path(),
        json!({"session_id":"session-claude-cross-action-source-access","transcript_path":root.as_path().join("session.jsonl"),"cwd":root.as_path(),"hook_event_name":"PreToolUse","tool_use_id":"toolu_read_source","tool_name":"Read","tool_input":{"file_path":root.as_path().join("src/lib.rs")}}),
        &[],
    );
    assert_eq!(first["hookSpecificOutput"]["permissionDecision"], "deny");
    let first_reason = first["hookSpecificOutput"]["permissionDecisionReason"]
        .as_str()
        .expect("first permission reason");
    assert!(
        first_reason.starts_with("ASP denied source access (`raw-broad-search`)"),
        "{first_reason}"
    );
    assert_eq!(
        first_reason,
        "ASP denied source access (`raw-broad-search`). Next: resume resident `asp-explore` for parser-owned ASP search."
    );
    let first_context = first["hookSpecificOutput"]["additionalContext"]
        .as_str()
        .expect("first decision context");
    assert!(first_context.contains("source-access-recovery"));
    let second_reason = second["hookSpecificOutput"]["permissionDecisionReason"]
        .as_str()
        .expect("second permission reason");
    assert!(
        second_reason.starts_with("ASP denied source access again (`direct-source-read`)"),
        "{second_reason}"
    );
    let context = second["hookSpecificOutput"]["additionalContext"]
        .as_str()
        .expect("decision context");
    assert!(context.contains("\"denyReplay\":\"repeated\""));
    assert!(context.contains("\"requiredAction\":\"enter-asp-explore-choice-pane\""));
    assert!(context.contains("\"nextAction\":\"choose-one-bootstrap-pane-option\""));
    assert!(context.contains("\"forbiddenUntilResolved\":\"raw-source-fallback\""));
}
