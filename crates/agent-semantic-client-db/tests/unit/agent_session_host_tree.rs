use crate::agent_session_registry::{
    AgentSessionInteractiveMenu, AgentSessionLoopState, AgentSessionRecord,
    ResidentChildBootstrapMenuInput, resident_child_bootstrap_menu,
    resident_child_host_tree_observation_menu,
};

fn live_resident() -> AgentSessionRecord {
    AgentSessionRecord {
        physical_generation: 1,
        configured_agent_type: Some("asp_explorer".into()),
        profile_evidence_json: None,
        project_id: "project".into(),
        root_session_id: "root".into(),
        session_id: "child".into(),
        message_target_id: Some("/root/asp_explorer".into()),
        parent_session_id: Some("root".into()),
        name: "asp-explore".into(),
        role: "asp_explorer".into(),
        model: Some("gpt-5.4-mini".into()),
        model_observation_source: Some("codex.subagent-start".into()),
        model_observed_at: Some(1),
        model_evidence_ref: Some("subagent-start:child".into()),
        status: "idle".into(),
        created_at: 1,
        updated_at: 1,
        last_seen_at: Some(1),
        last_heartbeat_at: Some(1),
        expires_at: None,
        archived_at: None,
        last_tool_event: None,
        last_command: None,
        last_evidence_ref: None,
        metadata_json: "{}".into(),
    }
}

fn menu(record: &AgentSessionRecord) -> AgentSessionInteractiveMenu<'_> {
    resident_child_bootstrap_menu(ResidentChildBootstrapMenuInput {
        platform: "codex",
        name: "asp-explore",
        root_session_id: Some("root"),
        record: Some(record),
        expected_model: Some("gpt-5.4-mini"),
        expected_reasoning_effort: Some("low"),
        rollout_history_status: None,
        rollout_history_action: None,
        now: 2,
    })
}

#[test]
fn hidden_but_routable_child_is_probed_before_replacement() {
    let record = live_resident();
    let absent =
        resident_child_host_tree_observation_menu(menu(&record), "absent", Some("present"));
    assert_eq!(absent.state, AgentSessionLoopState::Audit);
    assert_eq!(absent.choices.len(), 1);
    assert_eq!(
        absent.choices[0].id,
        "probe-hidden-routable-child-before-replacement"
    );
    assert!(
        absent
            .choices
            .iter()
            .all(|choice| choice.id != "create-canonical-typed-child-after-orphaned-owner")
    );

    let unroutable =
        resident_child_host_tree_observation_menu(menu(&record), "unroutable", Some("present"));
    assert_eq!(unroutable.state, AgentSessionLoopState::Audit);
    assert_eq!(unroutable.choices.len(), 1);
    assert_eq!(
        unroutable.choices[0].id,
        "create-canonical-typed-child-after-orphaned-owner"
    );
}
