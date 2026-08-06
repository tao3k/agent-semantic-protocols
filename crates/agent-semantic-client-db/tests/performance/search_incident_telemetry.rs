use std::{path::PathBuf, time::SystemTime};

use agent_semantic_client_db::{
    runtime_server_opentelemetry::{RuntimeServerOpenTelemetry, TursoOpenTelemetrySpanExporter},
    runtime_telemetry_bus::RuntimeTelemetryBus,
    search_incident::{
        IncidentIdentity, IncidentObservation, IncidentState, IncidentSurface,
        IncidentTelemetryEvent, RequestedProjection, ResourceObservation,
    },
};

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
