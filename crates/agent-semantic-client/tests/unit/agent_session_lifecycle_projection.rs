use agent_semantic_client::agent_session_lifecycle_projection::{
    AGENT_SESSION_LIFECYCLE_PROJECTION_SCHEMA_ID, AgentSessionLifecycleProjection, BindingPhase,
    DispatchObservation, DispatchPhase, HostBindingFacts, HostBindingObservation,
    HostBindingProjection, ServerHealth, SessionLifecycleProjection, SessionPhase,
    WorkspaceServerProjection, project_dispatch, project_host_binding,
    session_phase_from_registry_status,
};

fn server(health: ServerHealth) -> WorkspaceServerProjection {
    WorkspaceServerProjection {
        workspace_identity: "workspace-example".to_owned(),
        health,
    }
}

fn session(phase: SessionPhase) -> SessionLifecycleProjection {
    SessionLifecycleProjection {
        session_id: Some("session-example".to_owned()),
        generation: Some(7),
        phase,
    }
}

fn binding(observation: HostBindingObservation) -> Result<HostBindingProjection, String> {
    project_host_binding(HostBindingFacts {
        recorded: true,
        generation: Some(7),
        child_session_id: Some("child-example".to_owned()),
        canonical_message_target: Some("agent://child-example".to_owned()),
        observation,
        termination_receipt_indexed: false,
        path_release_receipt_indexed: false,
    })
}

fn released_binding(generation: u64) -> HostBindingProjection {
    project_host_binding(HostBindingFacts {
        recorded: true,
        generation: Some(generation),
        child_session_id: Some("child-example".to_owned()),
        canonical_message_target: Some("agent://child-example".to_owned()),
        observation: HostBindingObservation::Absent,
        termination_receipt_indexed: true,
        path_release_receipt_indexed: true,
    })
    .expect("released binding")
}

#[test]
fn fresh_binding_and_equal_generations_authorize_durable_dispatch() {
    let projection = AgentSessionLifecycleProjection::new(
        server(ServerHealth::Ready),
        session(SessionPhase::Active),
        binding(HostBindingObservation::PresentFresh).expect("fresh binding"),
        project_dispatch(Some(7), DispatchObservation::Idle),
    );
    assert!(projection.durable_dispatch_authorized);
}

#[test]
fn server_unavailability_does_not_revoke_durable_authority() {
    let projection = AgentSessionLifecycleProjection::new(
        server(ServerHealth::Unavailable),
        session(SessionPhase::Active),
        binding(HostBindingObservation::PresentFresh).expect("fresh binding"),
        project_dispatch(Some(7), DispatchObservation::Idle),
    );
    assert!(projection.durable_dispatch_authorized);
}

#[test]
fn recorded_binding_without_generation_is_rejected() {
    let error = project_host_binding(HostBindingFacts {
        recorded: true,
        generation: None,
        child_session_id: Some("child-example".to_owned()),
        canonical_message_target: Some("agent://child-example".to_owned()),
        observation: HostBindingObservation::PresentFresh,
        termination_receipt_indexed: false,
        path_release_receipt_indexed: false,
    })
    .expect_err("recorded binding without generation must fail");
    assert!(error.contains("physical generation"));
}

#[test]
fn missing_observation_is_not_unbound_or_idle() {
    assert_eq!(
        binding(HostBindingObservation::Unobserved)
            .expect("unobserved binding")
            .phase,
        BindingPhase::Unobserved
    );
    assert_eq!(
        project_dispatch(None, DispatchObservation::Unobserved).phase,
        DispatchPhase::Unobserved
    );
}

#[test]
fn observed_absence_makes_a_recorded_binding_stale() {
    assert_eq!(
        binding(HostBindingObservation::Absent)
            .expect("absent binding")
            .phase,
        BindingPhase::Stale
    );
}

#[test]
fn path_release_without_termination_is_rejected() {
    let error = project_host_binding(HostBindingFacts {
        recorded: true,
        generation: Some(7),
        child_session_id: Some("child-example".to_owned()),
        canonical_message_target: Some("agent://child-example".to_owned()),
        observation: HostBindingObservation::Absent,
        termination_receipt_indexed: false,
        path_release_receipt_indexed: true,
    })
    .expect_err("release without termination must fail");
    assert!(error.contains("termination receipt"));
}

#[test]
fn conflated_flat_registry_statuses_are_rejected() {
    for status in ["idle", "invalid", "orphan-risk"] {
        assert!(session_phase_from_registry_status(Some(status)).is_err());
    }
}

#[test]
fn missing_registry_observation_is_not_a_declared_session() {
    assert_eq!(
        session_phase_from_registry_status(None).expect("missing observation"),
        SessionPhase::Unobserved
    );
}

#[test]
fn fact_adapter_preserves_a_fully_unobserved_product() {
    let projection = project_agent_session_lifecycle(AgentSessionLifecycleFacts {
        workspace_identity: "workspace-example".to_owned(),
        server_health: ServerHealth::Unobserved,
        session_id: None,
        session_generation: None,
        registry_status: None,
        host_binding: HostBindingFacts {
            recorded: false,
            generation: None,
            child_session_id: None,
            canonical_message_target: None,
            observation: HostBindingObservation::Unobserved,
            termination_receipt_indexed: false,
            path_release_receipt_indexed: false,
        },
        dispatch_generation: None,
        dispatch_observation: DispatchObservation::Unobserved,
    })
    .expect("unobserved product");
    assert_eq!(projection.workspace_server.health, ServerHealth::Unobserved);
    assert_eq!(projection.session.phase, SessionPhase::Unobserved);
    assert_eq!(projection.host_binding.phase, BindingPhase::Unbound);
    assert_eq!(projection.dispatch.phase, DispatchPhase::Unobserved);
    assert!(!projection.durable_dispatch_authorized);
}

#[test]
fn archived_session_without_release_cannot_be_replaced() {
    let projection = AgentSessionLifecycleProjection::new(
        server(ServerHealth::Ready),
        session(SessionPhase::Archived),
        binding(HostBindingObservation::PresentStale).expect("stale binding"),
        project_dispatch(Some(7), DispatchObservation::Completed),
    );
    assert!(!projection.replacement_admitted(8));
}

#[test]
fn archived_released_session_admits_only_a_newer_generation() {
    let projection = AgentSessionLifecycleProjection::new(
        server(ServerHealth::Ready),
        session(SessionPhase::Archived),
        released_binding(7),
        project_dispatch(Some(7), DispatchObservation::Completed),
    );
    assert!(projection.replacement_admitted(8));
    assert!(!projection.replacement_admitted(7));
}

#[test]
fn stale_binding_generation_cannot_authorize_replacement() {
    let projection = AgentSessionLifecycleProjection::new(
        server(ServerHealth::Ready),
        session(SessionPhase::Archived),
        released_binding(6),
        project_dispatch(Some(7), DispatchObservation::Completed),
    );
    assert!(!projection.replacement_admitted(8));
}

#[test]
fn serialized_projection_uses_v1_contract_names() {
    let projection = AgentSessionLifecycleProjection::new(
        server(ServerHealth::Ready),
        session(SessionPhase::Active),
        binding(HostBindingObservation::PresentFresh).expect("fresh binding"),
        project_dispatch(Some(7), DispatchObservation::Idle),
    );
    let value = serde_json::to_value(&projection).expect("serialize projection");
    assert_eq!(
        value["schemaId"],
        AGENT_SESSION_LIFECYCLE_PROJECTION_SCHEMA_ID
    );
    assert_eq!(value["schemaVersion"], "1");
    assert_eq!(value["hostBinding"]["phase"], "fresh");
    assert_eq!(value["dispatch"]["phase"], "idle");
    let round_trip: AgentSessionLifecycleProjection =
        serde_json::from_value(value).expect("deserialize projection");
    assert_eq!(round_trip, projection);
}
use agent_semantic_client::agent_session_lifecycle_projection::{
    AgentSessionLifecycleFacts, project_agent_session_lifecycle,
};
