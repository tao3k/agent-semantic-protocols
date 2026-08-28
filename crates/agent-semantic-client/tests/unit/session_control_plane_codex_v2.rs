use super::{
    CodexV2HostEventKind, CodexV2HostLifecycleEvent, DurableAgentRunState, DurableChoiceAction,
    DurableMaterializationError, durable_choice, focused_child_admitted,
    materialize_codex_v2_host_event,
};

fn event(sequence: u64, event_kind: CodexV2HostEventKind) -> CodexV2HostLifecycleEvent {
    CodexV2HostLifecycleEvent {
        host_event_id: format!("host-event-{sequence}"),
        host_event_sequence: sequence,
        event_kind,
        root_session_id: "root-session".to_owned(),
        parent_session_id: "parent-session".to_owned(),
        child_session_id: "child-session".to_owned(),
        agent_name: "@asp_testing".to_owned(),
        namespace_id: "asp-testing".to_owned(),
        observed_at_unix_ms: 1,
    }
}

#[test]
fn create_is_only_selected_when_namespace_is_absent() {
    assert_eq!(durable_choice(None), DurableChoiceAction::CreateAndRegister);
}

#[test]
fn empty_or_unknown_host_event_payload_fails_closed() {
    assert!(serde_json::from_value::<CodexV2HostLifecycleEvent>(serde_json::json!({})).is_err());
    let mut value = serde_json::to_value(event(1, CodexV2HostEventKind::SubagentStart))
        .expect("serialize event");
    value
        .as_object_mut()
        .expect("event object")
        .insert("legacyPayload".to_owned(), serde_json::json!(true));
    assert!(serde_json::from_value::<CodexV2HostLifecycleEvent>(value).is_err());
}

#[test]
fn stopped_unachieved_namespace_is_resumed_without_new_generation() {
    let started =
        materialize_codex_v2_host_event(None, &event(1, CodexV2HostEventKind::SubagentStart))
            .expect("materialize start");
    let stopped = materialize_codex_v2_host_event(
        Some(&started),
        &event(2, CodexV2HostEventKind::SubagentStop),
    )
    .expect("materialize stop");

    assert_eq!(durable_choice(Some(&stopped)), DurableChoiceAction::Resume);
    let resumed = materialize_codex_v2_host_event(
        Some(&stopped),
        &event(3, CodexV2HostEventKind::SubagentResume),
    )
    .expect("materialize resume");
    assert_eq!(resumed.generation, started.generation);
    assert_eq!(resumed.run_state, DurableAgentRunState::Active);
}

#[test]
fn achieved_namespace_is_terminal_and_cannot_be_recreated_or_resumed() {
    let started =
        materialize_codex_v2_host_event(None, &event(1, CodexV2HostEventKind::SubagentStart))
            .expect("materialize start");
    let achieved = materialize_codex_v2_host_event(
        Some(&started),
        &event(2, CodexV2HostEventKind::SubagentAchieved),
    )
    .expect("materialize achieved");

    assert_eq!(
        durable_choice(Some(&achieved)),
        DurableChoiceAction::NoAction
    );
    assert_eq!(
        materialize_codex_v2_host_event(
            Some(&achieved),
            &event(3, CodexV2HostEventKind::SubagentResume),
        ),
        Err(DurableMaterializationError::NamespaceAchieved)
    );
}

#[test]
fn host_event_sequence_is_strictly_increasing() {
    let started =
        materialize_codex_v2_host_event(None, &event(2, CodexV2HostEventKind::SubagentStart))
            .expect("materialize start");
    assert_eq!(
        materialize_codex_v2_host_event(
            Some(&started),
            &event(2, CodexV2HostEventKind::SubagentStop),
        ),
        Err(DurableMaterializationError::EventSequenceNotIncreasing)
    );
}

#[test]
fn duplicate_host_delivery_is_idempotent() {
    let start = event(1, CodexV2HostEventKind::SubagentStart);
    let started = materialize_codex_v2_host_event(None, &start).expect("materialize start");
    let replayed =
        materialize_codex_v2_host_event(Some(&started), &start).expect("replay same event");
    assert_eq!(replayed, started);
}

#[test]
fn same_sequence_with_different_event_identity_is_rejected() {
    let started =
        materialize_codex_v2_host_event(None, &event(1, CodexV2HostEventKind::SubagentStart))
            .expect("materialize start");
    let mut drifted = event(1, CodexV2HostEventKind::SubagentStop);
    drifted.host_event_id = "different-host-event".to_owned();
    assert_eq!(
        materialize_codex_v2_host_event(Some(&started), &drifted),
        Err(DurableMaterializationError::EventSequenceNotIncreasing)
    );
}

#[test]
fn existing_namespace_rejects_a_second_start() {
    let started =
        materialize_codex_v2_host_event(None, &event(1, CodexV2HostEventKind::SubagentStart))
            .expect("materialize start");
    assert_eq!(
        materialize_codex_v2_host_event(
            Some(&started),
            &event(2, CodexV2HostEventKind::SubagentStart),
        ),
        Err(DurableMaterializationError::UnexpectedStartForExistingNamespace)
    );
}

#[test]
fn focused_registered_lanes_deny_undeclared_unregistered_nested_children() {
    assert!(!focused_child_admitted(true, false, false));
    assert!(focused_child_admitted(true, true, false));
    assert!(focused_child_admitted(true, false, true));
}
