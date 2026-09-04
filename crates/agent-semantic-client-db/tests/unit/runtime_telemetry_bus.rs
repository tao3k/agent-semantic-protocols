use agent_semantic_client_db::runtime_server_opentelemetry::RuntimeLifecycleEvent;
use agent_semantic_client_db::runtime_telemetry_bus::CAPACITY;
use agent_semantic_client_db::runtime_telemetry_bus::ResidentReadTerminalContext;
use agent_semantic_client_db::runtime_telemetry_bus::ResidentReadTerminalOutcome;
use agent_semantic_client_db::runtime_telemetry_bus::RuntimeTelemetryBus;
use agent_semantic_client_db::runtime_telemetry_bus::RuntimeTelemetryEvent;
use agent_semantic_client_db::workspace_db_ipc::WorkspaceIpcResidentReadWorkCounters;

fn event(transition: &str, state: &str) -> RuntimeLifecycleEvent {
    RuntimeLifecycleEvent {
        owner_epoch: 1,
        workspace_identity: Some("ws".to_owned()),
        generation_digest: None,
        candidate_digest: None,
        transition: transition.to_owned(),
        state: state.to_owned(),
        elapsed_micros: 1,
        read_bytes: 0,
        retained_bytes: 0,
        active_task_count: 1,
        active_child_count: 0,
    }
}

#[tokio::test]
async fn exact_selector_terminal_projects_query_runtime_latency() {
    let bus = RuntimeTelemetryBus::new();
    let context = ResidentReadTerminalContext {
        operation_id: "query-1".to_owned(),
        surface: "runtime-resident-runtime-selector".to_owned(),
        workspace_identity: "workspace-1".to_owned(),
        generation_digest: "blake3-256:generation".to_owned(),
        root_digest: "blake3-256:root".to_owned(),
        read_state:
            agent_semantic_client_db::workspace_db_ipc::RuntimeResidentReadState::Projection,
        elapsed_micros: 9,
        work_counters: WorkspaceIpcResidentReadWorkCounters {
            database_opens: 0,
            filesystem_reads: 0,
            provider_spawns: 0,
            control_socket_roundtrips: 0,
        },
    };
    let observation = context
        .clone()
        .into_performance_observation()
        .expect("exact selector observation");

    assert_eq!(observation.surface, "query");
    assert_eq!(observation.stage, "runtime-selector-read");
    assert_eq!(observation.elapsed_micros, 9);
    assert_eq!(observation.budget_micros, 1_000);
    assert_eq!(observation.budget_status, "within-budget");
    assert_eq!(observation.memory_search_turso_opens, Some(0));
    assert_eq!(observation.memory_search_source_bytes_read, Some(0));
    assert_eq!(observation.memory_search_provider_spawns, Some(0));
    assert_eq!(observation.memory_search_socket_connects, Some(0));

    bus.sender
        .try_record_resident_read_terminal(
            context,
            ResidentReadTerminalOutcome {
                terminal_state: agent_semantic_client_db::workspace_db_ipc::RuntimeResidentReadTerminalState::Success,
            },
        )
        .expect("resident Query terminal enters the Runtime telemetry bus");
    let mut receiver = bus.receiver;
    let RuntimeTelemetryEvent::Performance(recorded) =
        receiver.recv().await.expect("Query performance event")
    else {
        panic!("expected Query performance event");
    };
    assert_eq!(recorded.surface, "query");
    assert_eq!(recorded.elapsed_micros, 9);
}

#[tokio::test]
async fn terminal_events_preserve_transition_order() {
    let bus = RuntimeTelemetryBus::new();
    assert!(
        bus.sender
            .try_send_transition(event("building", "building"))
    );
    bus.sender
        .send_terminal(event("ready", "ready"))
        .await
        .expect("receiver is live");
    let mut receiver = bus.receiver;
    assert_eq!(
        match receiver.recv().await.expect("building") {
            RuntimeTelemetryEvent::Lifecycle(event) => event.transition,
            RuntimeTelemetryEvent::SearchIncident(_) => panic!("unexpected incident"),
            RuntimeTelemetryEvent::Performance(_) => panic!("unexpected performance event"),
        },
        "building"
    );
    assert_eq!(
        match receiver.recv().await.expect("ready") {
            RuntimeTelemetryEvent::Lifecycle(event) => event.transition,
            RuntimeTelemetryEvent::SearchIncident(_) => panic!("unexpected incident"),
            RuntimeTelemetryEvent::Performance(_) => panic!("unexpected performance event"),
        },
        "ready"
    );
}

#[tokio::test]
async fn sender_fails_closed_after_receiver_drop() {
    let bus = RuntimeTelemetryBus::new();
    drop(bus.receiver);
    assert!(!bus.sender.try_send_transition(event("failed", "failed")));
    assert!(
        bus.sender
            .send_terminal(event("failed", "failed"))
            .await
            .is_err()
    );
}

#[tokio::test]
async fn bounded_terminal_delivery_is_lossless_under_pressure() {
    let bus = RuntimeTelemetryBus::new();
    let sender = bus.sender.clone();
    let producer = tokio::spawn(async move {
        for index in 0..(CAPACITY + 16) {
            sender
                .send_terminal(event(&format!("terminal-{index}"), "ready"))
                .await
                .expect("terminal receiver remains live");
        }
    });
    let mut receiver = bus.receiver;
    let mut count = 0;
    while count < CAPACITY + 16 {
        receiver.recv().await.expect("terminal event");
        count += 1;
    }
    producer.await.expect("producer completes");
    assert_eq!(count, CAPACITY + 16);
}

#[tokio::test]
async fn independent_runtime_buses_do_not_cross_deliver() {
    let first = RuntimeTelemetryBus::new();
    let second = RuntimeTelemetryBus::new();
    first
        .sender
        .send_terminal(event("first", "ready"))
        .await
        .expect("first receiver");
    second
        .sender
        .send_terminal(event("second", "ready"))
        .await
        .expect("second receiver");
    let mut first_receiver = first.receiver;
    let mut second_receiver = second.receiver;
    assert_eq!(
        match first_receiver.recv().await.expect("first event") {
            RuntimeTelemetryEvent::Lifecycle(event) => event.transition,
            RuntimeTelemetryEvent::SearchIncident(_) => panic!("unexpected incident"),
            RuntimeTelemetryEvent::Performance(_) => panic!("unexpected performance event"),
        },
        "first"
    );
    assert_eq!(
        match second_receiver.recv().await.expect("second event") {
            RuntimeTelemetryEvent::Lifecycle(event) => event.transition,
            RuntimeTelemetryEvent::SearchIncident(_) => panic!("unexpected incident"),
            RuntimeTelemetryEvent::Performance(_) => panic!("unexpected performance event"),
        },
        "second"
    );
}
