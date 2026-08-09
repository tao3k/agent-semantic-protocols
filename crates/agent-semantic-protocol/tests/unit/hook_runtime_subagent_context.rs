use super::{
    apply_codex_subagent_context, payload_has_complete_typed_agent_identity,
    payload_indicates_subagent_context,
};

#[test]
fn typed_rollout_identity_normalizes_subagent_policy_facts() {
    let mut payload = serde_json::json!({"tool_name": "Bash"});

    apply_codex_subagent_context(
        &mut payload,
        "child-session",
        "root-session",
        Some("parent-session"),
        "asp_testing",
    );

    assert!(payload_indicates_subagent_context(&payload));
    assert_eq!(payload["is_subagent"], true);
    assert_eq!(payload["agent_id"], "child-session");
    assert_eq!(payload["agent_type"], "asp_testing");
    assert_eq!(payload["parent_session_id"], "parent-session");
}

#[test]
fn typed_rollout_identity_uses_root_when_direct_parent_is_absent() {
    let mut payload = serde_json::json!({});

    apply_codex_subagent_context(
        &mut payload,
        "child-session",
        "root-session",
        None,
        "asp_explorer",
    );

    assert_eq!(payload["parent_session_id"], "root-session");
}

#[test]
fn subagent_flag_without_typed_identity_is_enriched_to_dispatch_fixed_point() {
    let mut payload = serde_json::json!({
        "is_subagent": true,
        "agent_id": "",
        "agent_type": "",
    });
    assert!(payload_indicates_subagent_context(&payload));
    assert!(!payload_has_complete_typed_agent_identity(&payload));

    apply_codex_subagent_context(
        &mut payload,
        "testing-child",
        "root-session",
        Some("parent-session"),
        "asp_testing",
    );

    assert!(payload_has_complete_typed_agent_identity(&payload));
    assert_eq!(payload["agent_id"], "testing-child");
    assert_eq!(payload["agent_type"], "asp_testing");
}
