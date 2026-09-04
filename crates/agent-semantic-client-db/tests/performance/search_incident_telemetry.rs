use std::path::PathBuf;
use std::time::SystemTime;

use agent_semantic_client_db::runtime_server_opentelemetry::RuntimeServerOpenTelemetry;
use agent_semantic_client_db::runtime_server_opentelemetry::TursoOpenTelemetrySpanExporter;
use agent_semantic_client_db::runtime_telemetry_bus::RuntimeTelemetryBus;
use agent_semantic_client_db::search_incident::IncidentIdentity;
use agent_semantic_client_db::search_incident::IncidentObservation;
use agent_semantic_client_db::search_incident::IncidentState;
use agent_semantic_client_db::search_incident::IncidentSurface;
use agent_semantic_client_db::search_incident::IncidentTelemetryEvent;
use agent_semantic_client_db::search_incident::RequestedProjection;
use agent_semantic_client_db::search_incident::ResourceObservation;

fn socket_path(label: &str) -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .expect("system clock")
        .as_nanos();
    PathBuf::from(format!(
        "/tmp/asp-incident-{label}-{}-{nonce}.sock",
        std::process::id()
    ))
}

fn event(
    workspace_identity: &str,
    state: IncidentState,
    transition: &str,
    sequence: u64,
) -> IncidentTelemetryEvent {
    IncidentTelemetryEvent {
        observation: IncidentObservation {
            identity: IncidentIdentity {
                incident_id: "blake3-256:incident".to_owned(),
                workspace_identity: workspace_identity.to_owned(),
                language_id: "rust".to_owned(),
                surface: IncidentSurface::Query,
                canonical_request_digest: "blake3-256:request".to_owned(),
                requested_projection: RequestedProjection::Source,
                stage: "exact-projection".to_owned(),
                reason_kind: "selector-not-in-active-generation".to_owned(),
                runtime_artifact_digest: Some("blake3-256:runtime".to_owned()),
                provider_contract_digest: Some("blake3-256:provider".to_owned()),
                generation_digest: Some("blake3-256:generation".to_owned()),
            },
            succeeded: matches!(state, IncidentState::Resolved),
            blocked: !matches!(state, IncidentState::Resolved),
            budget_exceeded: false,
            code_projection_present: matches!(state, IncidentState::Resolved),
            budget_micros: Some(1_000),
            elapsed_micros: Some(900),
            observed_at_unix_micros: sequence,
            resources: ResourceObservation::default(),
        },
        state,
        transition: transition.to_owned(),
        transition_sequence: sequence,
    }
}

async fn persist(
    database_path: PathBuf,
    event: IncidentTelemetryEvent,
    label: &str,
) -> Result<(), String> {
    let bus = RuntimeTelemetryBus::new();
    let telemetry = RuntimeServerOpenTelemetry::start_with_telemetry_receiver(
        database_path,
        socket_path(&format!("{label}-ingress")),
        socket_path(&format!("{label}-query")),
        bus.receiver,
    )
    .await?;
    bus.sender
        .try_record_incident_terminal(event)
        .map_err(|_| "resident telemetry receiver rejected a valid terminal incident".to_owned())?;
    telemetry.shutdown().await.map(|_| ())
}

#[tokio::test]
async fn turso_active_incident_view_closes_only_after_resolved_transition() {
    let fixture = tempfile::tempdir().expect("fixture directory");
    let database_path = fixture.path().join("performance.turso");

    persist(
        database_path.clone(),
        event(
            "workspace-incident-fixture",
            IncidentState::Open,
            "failure-observed",
            1,
        ),
        "open",
    )
    .await
    .expect("persist open incident");
    let exporter = TursoOpenTelemetrySpanExporter::open(&database_path)
        .await
        .expect("open telemetry exporter");
    let active = exporter
        .active_search_incidents("workspace-incident-fixture", 10)
        .await
        .expect("query active incidents");
    assert_eq!(active.len(), 1);
    assert_eq!(active[0].incident_state, "open");
    assert_eq!(active[0].incident_transition, "failure-observed");
    assert_eq!(active[0].requested_projection.as_deref(), Some("source"));

    let mut warm_query_micros = Vec::with_capacity(64);
    for _ in 0..64 {
        let started = tokio::time::Instant::now();
        let replay = exporter
            .active_search_incidents("workspace-incident-fixture", 10)
            .await
            .expect("warm active incident query");
        warm_query_micros.push(started.elapsed().as_micros() as u64);
        assert_eq!(replay.len(), 1);
    }
    warm_query_micros.sort_unstable();
    let p95 = warm_query_micros[(warm_query_micros.len() * 95).div_ceil(100) - 1];
    eprintln!(
        "search-incident-active-query samples=64 p95Micros={p95} maxMicros={}",
        warm_query_micros.last().copied().unwrap_or_default()
    );
    assert!(
        p95 <= 1_000,
        "resident active incident Turso query p95 exceeded 1ms: p95Micros={p95} samples={warm_query_micros:?}"
    );

    persist(
        database_path.clone(),
        event(
            "workspace-independent-fixture",
            IncidentState::Open,
            "failure-observed",
            1,
        ),
        "independent",
    )
    .await
    .expect("persist independent workspace incident");

    persist(
        database_path.clone(),
        event(
            "workspace-incident-fixture",
            IncidentState::Resolved,
            "verification-succeeded",
            2,
        ),
        "resolved",
    )
    .await
    .expect("persist resolved incident");
    let active = exporter
        .active_search_incidents("workspace-incident-fixture", 10)
        .await
        .expect("query resolved incidents");
    assert!(active.is_empty());
    let independent = exporter
        .active_search_incidents("workspace-independent-fixture", 10)
        .await
        .expect("query independent workspace incidents");
    assert_eq!(independent.len(), 1);
    assert_eq!(independent[0].incident_state, "open");
}

#[test]
fn typed_terminal_adapter_deduplicates_and_sequences_failures() {
    use agent_semantic_client_db::search_incident::SearchIncidentTerminalContext;
    use agent_semantic_client_db::search_incident::SearchIncidentTerminalOutcome;
    use agent_semantic_client_db::search_incident::observe_terminal;

    let context = SearchIncidentTerminalContext {
        workspace_identity: "workspace-terminal-e2e".to_owned(),
        language_id: "rust".to_owned(),
        surface: IncidentSurface::Query,
        canonical_request_digest: "blake3-256:terminal-request".to_owned(),
        requested_projection: RequestedProjection::Source,
        stage: "exact-projection".to_owned(),
        runtime_artifact_digest: Some("blake3-256:runtime".to_owned()),
        provider_contract_digest: Some("blake3-256:provider".to_owned()),
        generation_digest: Some("blake3-256:generation".to_owned()),
        budget_micros: Some(1_000),
        elapsed_micros: Some(900),
        observed_at_unix_micros: 1,
        resources: ResourceObservation::default(),
    };
    let outcome = SearchIncidentTerminalOutcome::Blocked {
        reason_kind: "generation-unavailable".to_owned(),
    };
    let (first, first_event) = observe_terminal(None, context.clone(), outcome.clone())
        .expect("first terminal observation")
        .expect("first incident");
    let (second, second_event) = observe_terminal(Some(first), context, outcome)
        .expect("repeated terminal observation")
        .expect("repeated incident");
    assert_eq!(second.occurrence_count, 2);
    assert_eq!(first_event.transition_sequence, 1);
    assert_eq!(second_event.transition_sequence, 2);
    assert_eq!(second_event.transition, "repeated-failure");
}

#[tokio::test]
async fn terminal_incident_lane_survives_transition_queue_pressure() {
    use agent_semantic_client_db::runtime_server_opentelemetry::RuntimeLifecycleEvent;
    use agent_semantic_client_db::runtime_telemetry_bus::CAPACITY;
    use agent_semantic_client_db::runtime_telemetry_bus::RuntimeTelemetryEvent;

    let mut bus = RuntimeTelemetryBus::new();
    for sequence in 0..CAPACITY {
        assert!(bus.sender.try_send_transition(RuntimeLifecycleEvent {
            owner_epoch: sequence as u64,
            workspace_identity: Some("workspace-pressure-e2e".to_owned()),
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
        .try_record_incident_terminal(event(
            "workspace-pressure-e2e",
            IncidentState::Open,
            "failure-observed",
            1,
        ))
        .expect("reserved terminal lane");
    assert!(matches!(
        bus.receiver.recv().await,
        Some(RuntimeTelemetryEvent::SearchIncident(_))
    ));
}

#[tokio::test]
async fn fresh_store_creates_materialized_active_incident_relation_before_indexes() {
    let fixture = tempfile::tempdir().expect("fixture directory");
    let database_path = fixture.path().join("fresh-performance.turso");
    let exporter = TursoOpenTelemetrySpanExporter::open(&database_path)
        .await
        .expect("fresh telemetry store migration");
    assert!(
        exporter
            .active_search_incidents("workspace-fresh", 1)
            .await
            .expect("fresh active incident relation")
            .is_empty()
    );
}

#[tokio::test]
async fn legacy_active_incident_view_is_replaced_by_materialized_table() {
    let fixture = tempfile::tempdir().expect("fixture directory");
    let database_path = fixture.path().join("legacy-view-performance.turso");
    let database = turso::Builder::new_local(
        database_path
            .to_str()
            .expect("legacy telemetry database path must be UTF-8"),
    )
    .build()
    .await
    .expect("legacy telemetry database");
    let connection = database.connect().expect("legacy telemetry connection");
    connection
        .execute(
            "CREATE VIEW asp_otel_active_search_incident AS
             SELECT 'legacy' AS workspace_identity",
            (),
        )
        .await
        .expect("legacy active incident view");
    drop(connection);
    drop(database);

    let exporter = TursoOpenTelemetrySpanExporter::open(&database_path)
        .await
        .expect("legacy active incident relation migration");
    assert!(
        exporter
            .active_search_incidents("workspace-legacy", 1)
            .await
            .expect("materialized active incident relation")
            .is_empty()
    );
}

#[tokio::test]
async fn telemetry_persistence_bootstrap_failure_is_non_fatal_to_runtime_lane() {
    let fixture = tempfile::tempdir().expect("fixture directory");
    let bus = RuntimeTelemetryBus::new();
    let telemetry = RuntimeServerOpenTelemetry::start_with_telemetry_receiver(
        fixture.path().to_path_buf(),
        socket_path("fail-soft-ingress"),
        socket_path("fail-soft-query"),
        bus.receiver,
    )
    .await
    .expect("telemetry persistence failure must not reject Runtime startup");
    telemetry
        .shutdown()
        .await
        .expect("degraded telemetry lane must drain cleanly");
}
