use agent_semantic_client::agent_session_lifecycle_projection::{
    AGENT_SESSION_LIFECYCLE_PROJECTION_SCHEMA_ID, AgentSessionLifecycleProjection, BindingPhase,
    DispatchObservation, DispatchPhase, HostBindingFacts, HostBindingObservation,
    HostBindingProjection, RequiredDispatchAction, ServerHealth, SessionLifecycleProjection,
    SessionPhase, WorkspaceServerProjection, project_dispatch, project_host_binding,
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
    })
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
        },
        dispatch_generation: None,
        dispatch_observation: DispatchObservation::Unobserved,
    })
    .expect("unobserved product");
    assert_eq!(projection.workspace_server.health, ServerHealth::Unobserved);
    assert_eq!(projection.session.phase, SessionPhase::Unobserved);
    assert_eq!(projection.host_binding.phase, BindingPhase::Unbound);
    assert_eq!(projection.dispatch.phase, DispatchPhase::Unobserved);
    assert_eq!(
        projection.required_dispatch_action,
        RequiredDispatchAction::Unavailable
    );
    assert!(!projection.durable_dispatch_authorized);
}

#[test]
fn interrupted_present_agent_uses_followup_and_rejects_spawn() {
    let projection = AgentSessionLifecycleProjection::new(
        server(ServerHealth::Ready),
        session(SessionPhase::Interrupted),
        binding(HostBindingObservation::PresentFresh).expect("fresh binding"),
        project_dispatch(Some(7), DispatchObservation::Completed),
    );
    assert_eq!(
        projection.required_dispatch_action,
        RequiredDispatchAction::FollowupTask
    );
    assert!(projection.followup_task_admitted());
    assert!(!projection.spawn_agent_admitted());
}

#[test]
fn observed_absent_agent_uses_spawn_and_rejects_followup() {
    let projection = AgentSessionLifecycleProjection::new(
        server(ServerHealth::Ready),
        session(SessionPhase::Completed),
        binding(HostBindingObservation::Absent).expect("absent binding"),
        project_dispatch(Some(7), DispatchObservation::Completed),
    );
    assert_eq!(
        projection.required_dispatch_action,
        RequiredDispatchAction::SpawnAgent
    );
    assert!(projection.spawn_agent_admitted());
    assert!(!projection.followup_task_admitted());
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
    assert_eq!(value["requiredDispatchAction"], "followup-task");
    let round_trip: AgentSessionLifecycleProjection =
        serde_json::from_value(value).expect("deserialize projection");
    assert_eq!(round_trip, projection);
}
use agent_semantic_client::agent_session_lifecycle_projection::{
    AgentSessionLifecycleFacts, project_agent_session_lifecycle,
};
