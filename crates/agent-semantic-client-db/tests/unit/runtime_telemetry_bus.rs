use agent_semantic_client_db::runtime_server_opentelemetry::RuntimeLifecycleEvent;
use agent_semantic_client_db::runtime_telemetry_bus::{
    CAPACITY, RuntimeTelemetryBus, RuntimeTelemetryEvent,
};

fn event(transition: &str, state: &str) -> RuntimeLifecycleEvent {
    RuntimeLifecycleEvent {
        owner_epoch: 1,
        workspace_identity: Some("ws".to_owned()),
        generation_digest: None,
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
        },
        "building"
    );
    assert_eq!(
        match receiver.recv().await.expect("ready") {
            RuntimeTelemetryEvent::Lifecycle(event) => event.transition,
            RuntimeTelemetryEvent::SearchIncident(_) => panic!("unexpected incident"),
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
        },
        "first"
    );
    assert_eq!(
        match second_receiver.recv().await.expect("second event") {
            RuntimeTelemetryEvent::Lifecycle(event) => event.transition,
            RuntimeTelemetryEvent::SearchIncident(_) => panic!("unexpected incident"),
        },
        "second"
    );
}
