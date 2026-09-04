use agent_semantic_client_db::search_incident::IncidentIdentity;
use agent_semantic_client_db::search_incident::IncidentObservation;
use agent_semantic_client_db::search_incident::IncidentState;
use agent_semantic_client_db::search_incident::IncidentSurface;
use agent_semantic_client_db::search_incident::ReplayReceipt;
use agent_semantic_client_db::search_incident::RequestedProjection;
use agent_semantic_client_db::search_incident::ResourceObservation;
use agent_semantic_client_db::search_incident::TransitionError;
use agent_semantic_client_db::search_incident::apply_replay;
use agent_semantic_client_db::search_incident::begin_repair;
use agent_semantic_client_db::search_incident::compact;
use agent_semantic_client_db::search_incident::observe;
use agent_semantic_client_db::search_incident::reopen_failed_verification;
use agent_semantic_client_db::search_incident::request_verification;
use agent_semantic_client_db::search_incident::should_record;

fn observation(
    projection: RequestedProjection,
    succeeded: bool,
    code: bool,
) -> IncidentObservation {
    IncidentObservation {
        identity: IncidentIdentity {
            incident_id: "incident-1".to_owned(),
            workspace_identity: "workspace-1".to_owned(),
            language_id: "rust".to_owned(),
            surface: IncidentSurface::Query,
            canonical_request_digest: "request-digest".to_owned(),
            requested_projection: projection,
            stage: "exact-projection".to_owned(),
            reason_kind: "code-projection-missing".to_owned(),
            runtime_artifact_digest: Some("runtime-old".to_owned()),
            provider_contract_digest: Some("provider-1".to_owned()),
            generation_digest: Some("generation-1".to_owned()),
        },
        succeeded,
        blocked: false,
        budget_exceeded: false,
        code_projection_present: code,
        budget_micros: Some(1_000),
        elapsed_micros: Some(900),
        observed_at_unix_micros: 10,
        resources: ResourceObservation::default(),
    }
}

fn telemetry_event(
    workspace_identity: &str,
) -> agent_semantic_client_db::search_incident::IncidentTelemetryEvent {
    use agent_semantic_client_db::search_incident::IncidentIdentity;
    use agent_semantic_client_db::search_incident::IncidentObservation;
    use agent_semantic_client_db::search_incident::IncidentState;
    use agent_semantic_client_db::search_incident::IncidentSurface;
    use agent_semantic_client_db::search_incident::RequestedProjection;
    use agent_semantic_client_db::search_incident::ResourceObservation;

    agent_semantic_client_db::search_incident::IncidentTelemetryEvent {
        observation: IncidentObservation {
            identity: IncidentIdentity {
                incident_id: format!("incident-{workspace_identity}"),
                workspace_identity: workspace_identity.to_owned(),
                language_id: "rust".to_owned(),
                surface: IncidentSurface::Query,
                canonical_request_digest: "request-digest".to_owned(),
                requested_projection: RequestedProjection::Source,
                stage: "exact-projection".to_owned(),
                reason_kind: "selector-not-in-active-generation".to_owned(),
                runtime_artifact_digest: Some("runtime-digest".to_owned()),
                provider_contract_digest: Some("provider-digest".to_owned()),
                generation_digest: Some("generation-digest".to_owned()),
            },
            succeeded: false,
            blocked: true,
            budget_exceeded: true,
            code_projection_present: false,
            budget_micros: Some(1_000),
            elapsed_micros: Some(1_500),
            observed_at_unix_micros: 42,
            resources: ResourceObservation {
                resident_memory_bytes: Some(10),
                active_task_count: Some(2),
                writer_queue_depth: Some(3),
                ..ResourceObservation::default()
            },
        },
        state: IncidentState::Open,
        transition: "observed".to_owned(),
        transition_sequence: 1,
    }
}

#[tokio::test]
async fn telemetry_bus_delivers_terminal_incidents_losslessly() {
    use agent_semantic_client_db::runtime_telemetry_bus::RuntimeTelemetryBus;
    use agent_semantic_client_db::runtime_telemetry_bus::RuntimeTelemetryEvent;

    let mut bus = RuntimeTelemetryBus::new();
    let event = telemetry_event("workspace-a");
    bus.sender
        .try_record_incident_terminal(event.clone())
        .expect("terminal incident must be delivered while the resident receiver is alive");

    let received = bus.receiver.recv().await.expect("incident event");
    match received {
        RuntimeTelemetryEvent::SearchIncident(received) => assert_eq!(received, event),
        RuntimeTelemetryEvent::Lifecycle(_) => panic!("received lifecycle event"),
        RuntimeTelemetryEvent::Performance(_) => panic!("received performance event"),
    }
}

#[tokio::test]
async fn telemetry_bus_keeps_workspace_incidents_isolated() {
    use agent_semantic_client_db::runtime_telemetry_bus::RuntimeTelemetryBus;

    let mut first = RuntimeTelemetryBus::new();
    let mut second = RuntimeTelemetryBus::new();
    assert!(
        first
            .sender
            .try_send_incident_transition(telemetry_event("workspace-a"))
    );
    assert!(
        second
            .sender
            .try_send_incident_transition(telemetry_event("workspace-b"))
    );

    for (bus, expected) in [(&mut first, "workspace-a"), (&mut second, "workspace-b")] {
        let received = bus.receiver.recv().await.expect("incident event");
        let observation = received.into_observation();
        assert_eq!(observation.surface, "query");
        assert_eq!(observation.stage, "exact-projection");
        assert_eq!(observation.workspace_identity.as_deref(), Some(expected));
        assert_eq!(
            observation.failure_reason.as_deref(),
            Some("selector-not-in-active-generation")
        );
        assert_eq!(observation.process_resident_bytes, Some(10));
        assert_eq!(observation.runtime_alive_tasks, Some(2));
        assert_eq!(observation.runtime_global_queue_depth, Some(3));
        assert_eq!(observation.budget_status, "budget-exceeded");
        assert_eq!(observation.incident_state.as_deref(), Some("open"));
    }

    assert!(first.receiver.try_recv().is_err());
    assert!(second.receiver.try_recv().is_err());
}

#[tokio::test]
async fn telemetry_bus_rejects_unsequenced_incident_transitions() {
    use agent_semantic_client_db::runtime_telemetry_bus::RuntimeTelemetryBus;

    let mut bus = RuntimeTelemetryBus::new();
    let mut event = telemetry_event("workspace-a");
    event.transition_sequence = 0;
    assert!(!bus.sender.try_send_incident_transition(event.clone()));
    assert!(bus.sender.try_record_incident_terminal(event).is_err());
    assert!(bus.receiver.try_recv().is_err());
}

#[tokio::test]
async fn terminal_incident_lane_is_not_starved_by_transition_pressure() {
    use agent_semantic_client_db::runtime_server_opentelemetry::RuntimeLifecycleEvent;
    use agent_semantic_client_db::runtime_telemetry_bus::CAPACITY;
    use agent_semantic_client_db::runtime_telemetry_bus::RuntimeTelemetryBus;
    use agent_semantic_client_db::runtime_telemetry_bus::RuntimeTelemetryEvent;

    let mut bus = RuntimeTelemetryBus::new();
    for sequence in 0..CAPACITY {
        assert!(bus.sender.try_send_transition(RuntimeLifecycleEvent {
            owner_epoch: sequence as u64,
            workspace_identity: Some("workspace-pressure".to_owned()),
            generation_digest: None,
            candidate_digest: None,
            transition: "coalescable".to_owned(),
            state: "building".to_owned(),
            elapsed_micros: 0,
            read_bytes: 0,
            retained_bytes: 0,
            active_task_count: 0,
            active_child_count: 0,
        }));
    }
    bus.sender
        .try_record_incident_terminal(telemetry_event("workspace-pressure"))
        .expect("reserved terminal lane");
    assert!(matches!(
        bus.receiver.recv().await,
        Some(RuntimeTelemetryEvent::SearchIncident(_))
    ));
}

#[test]
fn lifecycle_owns_monotonic_transition_sequence_and_event_identity() {
    use agent_semantic_client_db::search_incident::IncidentTelemetryEvent;
    use agent_semantic_client_db::search_incident::ReplayReceipt;
    use agent_semantic_client_db::search_incident::apply_replay;
    use agent_semantic_client_db::search_incident::begin_repair;
    use agent_semantic_client_db::search_incident::observe;
    use agent_semantic_client_db::search_incident::reopen_failed_verification;
    use agent_semantic_client_db::search_incident::request_verification;

    let observation = telemetry_event("workspace-sequence").observation;
    let mut record = observe(None, observation.clone())
        .expect("observe transition")
        .expect("incident record");
    assert_eq!(record.transition_sequence, 1);
    let observed =
        IncidentTelemetryEvent::from_record(observation.clone(), &record, "failure-observed")
            .expect("typed observed transition");
    assert_eq!(observed.transition_sequence, 1);

    begin_repair(&mut record, "runtime-digest".to_owned()).expect("begin repair");
    assert_eq!(record.transition_sequence, 2);
    request_verification(&mut record).expect("request verification");
    assert_eq!(record.transition_sequence, 3);
    assert!(
        apply_replay(
            &mut record,
            ReplayReceipt {
                receipt_digest: "receipt-digest".to_owned(),
                canonical_request_digest: "request-digest".to_owned(),
                workspace_identity: "workspace-sequence".to_owned(),
                runtime_artifact_digest: "runtime-digest".to_owned(),
                generation_digest: "generation-digest".to_owned(),
                code_projection_present: true,
                within_budget: false,
                verified_at_unix_micros: 43,
            },
        )
        .is_err()
    );
    assert_eq!(record.transition_sequence, 4);
    reopen_failed_verification(&mut record).expect("reopen failed verification");
    assert_eq!(record.transition_sequence, 5);
}

fn terminal_context(
    workspace_identity: &str,
    requested_projection: agent_semantic_client_db::search_incident::RequestedProjection,
) -> agent_semantic_client_db::search_incident::SearchIncidentTerminalContext {
    use agent_semantic_client_db::search_incident::IncidentSurface;
    use agent_semantic_client_db::search_incident::ResourceObservation;

    agent_semantic_client_db::search_incident::SearchIncidentTerminalContext {
        workspace_identity: workspace_identity.to_owned(),
        language_id: "rust".to_owned(),
        surface: IncidentSurface::Query,
        canonical_request_digest: "request-digest".to_owned(),
        requested_projection,
        stage: "exact-projection".to_owned(),
        runtime_artifact_digest: Some("runtime-digest".to_owned()),
        provider_contract_digest: Some("provider-digest".to_owned()),
        generation_digest: Some("generation-digest".to_owned()),
        budget_micros: Some(1_000),
        elapsed_micros: Some(900),
        observed_at_unix_micros: 100,
        resources: ResourceObservation::default(),
    }
}

#[test]
fn typed_terminal_adapter_covers_all_failure_classes_without_false_success_incidents() {
    use agent_semantic_client_db::search_incident::RequestedProjection;
    use agent_semantic_client_db::search_incident::SearchIncidentTerminalOutcome;
    use agent_semantic_client_db::search_incident::observe_terminal;

    assert!(
        observe_terminal(
            None,
            terminal_context("workspace-terminal", RequestedProjection::Source),
            SearchIncidentTerminalOutcome::Succeeded {
                code_projection_present: true,
            },
        )
        .expect("source success")
        .is_none()
    );
    assert!(
        observe_terminal(
            None,
            terminal_context("workspace-terminal", RequestedProjection::Seeds),
            SearchIncidentTerminalOutcome::Succeeded {
                code_projection_present: false,
            },
        )
        .expect("non-code success")
        .is_none()
    );

    for outcome in [
        SearchIncidentTerminalOutcome::Failed {
            reason_kind: "provider-failed".to_owned(),
        },
        SearchIncidentTerminalOutcome::Blocked {
            reason_kind: "generation-unavailable".to_owned(),
        },
        SearchIncidentTerminalOutcome::BudgetExceeded {
            reason_kind: "wall-budget-exceeded".to_owned(),
            code_projection_present: false,
        },
        SearchIncidentTerminalOutcome::Succeeded {
            code_projection_present: false,
        },
    ] {
        let (record, event) = observe_terminal(
            None,
            terminal_context("workspace-terminal", RequestedProjection::Source),
            outcome,
        )
        .expect("typed terminal observation")
        .expect("active incident");
        assert_eq!(record.transition_sequence, 1);
        assert_eq!(event.transition_sequence, 1);
        assert_eq!(event.transition, "failure-observed");
    }
}

#[test]
fn repeated_terminal_failure_reuses_identity_and_advances_sequence() {
    use agent_semantic_client_db::search_incident::RequestedProjection;
    use agent_semantic_client_db::search_incident::SearchIncidentTerminalOutcome;
    use agent_semantic_client_db::search_incident::observe_terminal;

    let outcome = SearchIncidentTerminalOutcome::Blocked {
        reason_kind: "generation-unavailable".to_owned(),
    };
    let (first, first_event) = observe_terminal(
        None,
        terminal_context("workspace-terminal-repeat", RequestedProjection::Source),
        outcome.clone(),
    )
    .expect("first terminal observation")
    .expect("first incident");
    let (second, second_event) = observe_terminal(
        Some(first),
        terminal_context("workspace-terminal-repeat", RequestedProjection::Source),
        outcome,
    )
    .expect("repeated terminal observation")
    .expect("repeated incident");
    assert_eq!(
        first_event.observation.identity.incident_id,
        second.identity.incident_id
    );
    assert_eq!(second.occurrence_count, 2);
    assert_eq!(second.transition_sequence, 2);
    assert_eq!(second_event.transition, "repeated-failure");
}

#[test]
fn requested_code_without_code_creates_an_incident() {
    let observation = observation(RequestedProjection::Source, true, false);
    assert!(should_record(&observation));
    let record = observe(None, observation).unwrap().unwrap();
    assert_eq!(record.state, IncidentState::Open);
    assert_eq!(record.occurrence_count, 1);
}

#[test]
fn successful_non_code_projection_does_not_create_an_incident() {
    let observation = observation(RequestedProjection::Seeds, true, false);
    assert!(!should_record(&observation));
    assert!(observe(None, observation).unwrap().is_none());
}

#[test]
fn repeated_identity_updates_one_active_record() {
    let first = observation(RequestedProjection::Source, false, false);
    let record = observe(None, first).unwrap().unwrap();
    let mut repeated = observation(RequestedProjection::Source, false, false);
    repeated.observed_at_unix_micros = 20;
    let record = observe(Some(record), repeated).unwrap().unwrap();
    assert_eq!(record.occurrence_count, 2);
    assert_eq!(record.last_seen_unix_micros, 20);
}

#[test]
fn only_matching_code_bearing_replay_resolves_and_compacts() {
    let mut record = observe(
        None,
        observation(RequestedProjection::CallableSkeleton, false, false),
    )
    .unwrap()
    .unwrap();
    begin_repair(&mut record, "runtime-fixed".to_owned()).unwrap();
    request_verification(&mut record).unwrap();
    apply_replay(
        &mut record,
        ReplayReceipt {
            receipt_digest: "receipt-1".to_owned(),
            canonical_request_digest: "request-digest".to_owned(),
            workspace_identity: "workspace-1".to_owned(),
            runtime_artifact_digest: "runtime-fixed".to_owned(),
            generation_digest: "generation-1".to_owned(),
            code_projection_present: true,
            within_budget: true,
            verified_at_unix_micros: 30,
        },
    )
    .unwrap();
    assert_eq!(record.state, IncidentState::Resolved);
    assert!(!record.is_active());
    compact(&mut record).unwrap();
    assert_eq!(record.state, IncidentState::Compacted);
}

#[test]
fn mismatched_replay_fails_and_reopens() {
    let mut record = observe(None, observation(RequestedProjection::Source, false, false))
        .unwrap()
        .unwrap();
    begin_repair(&mut record, "runtime-fixed".to_owned()).unwrap();
    request_verification(&mut record).unwrap();
    let failure = apply_replay(
        &mut record,
        ReplayReceipt {
            receipt_digest: "receipt-2".to_owned(),
            canonical_request_digest: "wrong-request".to_owned(),
            workspace_identity: "workspace-1".to_owned(),
            runtime_artifact_digest: "runtime-fixed".to_owned(),
            generation_digest: "generation-1".to_owned(),
            code_projection_present: true,
            within_budget: true,
            verified_at_unix_micros: 30,
        },
    );
    assert_eq!(failure, Err(TransitionError::ReplayMismatch));
    assert_eq!(record.state, IncidentState::FailedVerification);
    reopen_failed_verification(&mut record).unwrap();
    assert_eq!(record.state, IncidentState::Open);
}
