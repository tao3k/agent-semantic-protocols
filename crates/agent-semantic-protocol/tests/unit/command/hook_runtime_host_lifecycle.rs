#[test]
fn codex_payload_identity_keeps_root_and_child_distinct() {
    let payload = serde_json::json!({
        "session_id": "root-1",
        "agent_id": "child-1"
    });
    assert_eq!(
        super::codex_payload_identity(&payload).expect("typed root and child identity"),
        ("root-1", "child-1")
    );

    let mismatch = serde_json::json!({
        "session_id": "child-1",
        "agent_id": "child-1"
    });
    assert_eq!(
        super::codex_payload_identity(&mismatch).expect_err("collapsed identity must fail closed"),
        "Codex Host lifecycle root and child must be distinct: root=child-1 child=child-1"
    );

    let missing = serde_json::json!({"session_id": "root-1"});
    assert_eq!(
        super::codex_payload_identity(&missing)
            .expect_err("missing stable child identity must fail closed"),
        "host-binding-evidence-unavailable: Codex lifecycle payload has no stable child identity"
    );
}

#[test]
fn codex_rollout_topology_verifies_root_and_preserves_nested_parent() {
    assert_eq!(
        super::verified_rollout_topology("child-2", "root-1", Some("root-1"), Some("child-1"))
            .expect("nested rollout parent"),
        ("child-1".to_owned(), true)
    );
    assert_eq!(
        super::verified_rollout_topology("child-1", "root-1", None, Some("root-1"))
            .expect("direct child parent edge supplies typed root evidence"),
        ("root-1".to_owned(), true)
    );
    assert_eq!(
        super::verified_rollout_topology("child-2", "root-1", None, Some("child-1"))
            .expect("nested child requires parent-chain verification"),
        ("child-1".to_owned(), false)
    );
    assert_eq!(
        super::verified_rollout_topology("child-1", "root-1", Some("root-2"), Some("root-1"))
            .expect_err("rollout mismatch must fail closed"),
        "Codex Host lifecycle root mismatch for child child-1: payload=root-1 rollout=root-2"
    );
    assert_eq!(
        super::verified_rollout_topology("child-1", "root-1", Some("root-1"), Some("child-1"))
            .expect_err("self-parent must fail closed"),
        "Codex Host lifecycle child cannot be its own parent: child=child-1"
    );
}

#[test]
fn only_native_subagent_start_and_stop_own_host_lifecycle() {
    let payload = serde_json::json!({
        "agent_id": "child-session",
        "agent_type": "asp_explorer",
    });
    assert_eq!(
        super::host_lifecycle_kind("codex", "pre-tool", &payload),
        None
    );
    assert_eq!(
        super::host_lifecycle_kind("codex", "subagent-stop", &payload),
        Some(agent_semantic_client_db::workspace_db_ipc::AgentHostLifecycleEventKind::Stopped)
    );
    assert_eq!(
        super::host_lifecycle_kind("codex", "subagent-start", &payload),
        Some(agent_semantic_client_db::workspace_db_ipc::AgentHostLifecycleEventKind::Started)
    );
}

#[test]
fn focused_registered_subagent_denies_nested_child_start() {
    let decision = super::focused_nested_subagent_denial(
        "codex",
        "subagent-start",
        "asp-testing-session",
        "nested-session",
        "explorer",
    );
    assert_eq!(decision.decision, agent_semantic_hook::DecisionKind::Deny);
    assert_eq!(
        decision.reason_kind,
        agent_semantic_hook::ReasonKind::FocusedSubagentNestedStart
    );
    assert_eq!(
        decision.fields["focusMode"],
        serde_json::Value::String("leaf".to_owned())
    );
    assert_eq!(
        decision.fields["parentCapability"],
        serde_json::Value::String("focused-leaf".to_owned())
    );
    assert_eq!(
        decision.fields["childSessionId"],
        serde_json::Value::String("nested-session".to_owned())
    );
    assert!(decision.message.contains("cannot start nested subagent"));
}

#[test]
fn root_pre_tool_is_not_a_child_lifecycle_event() {
    let payload = serde_json::json!({"session_id": "root-session"});
    assert_eq!(
        super::host_lifecycle_kind("codex", "pre-tool", &payload),
        None
    );
}
