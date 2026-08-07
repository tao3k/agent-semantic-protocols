use agent_semantic_client_db::workspace_db_ipc::AgentSessionRegistryIpcOperation;
use agent_semantic_client_db::{
    SessionControlPlaneAgentRegistration, SessionControlPlaneDelegationProposal,
};
use agent_semantic_context_product::agent_session_delegation_admission::AgentSessionDelegationCapability;

#[test]
fn control_plane_agent_registration_ipc_round_trips_without_implicit_defaults() {
    let operation = AgentSessionRegistryIpcOperation::RegisterControlPlaneAgent {
        registration: SessionControlPlaneAgentRegistration {
            project_id: "project-1".to_owned(),
            root_session_id: "root-1".to_owned(),
            session_id: "asp-testing-1".to_owned(),
            parent_session_id: Some("root-1".to_owned()),
            resident_name: "asp_testing".to_owned(),
            capability: AgentSessionDelegationCapability::FocusedLeaf,
        },
    };
    let value = serde_json::to_value(&operation).expect("encode registration operation");
    assert_eq!(value["kind"], "register-control-plane-agent");
    assert_eq!(value["registration"]["capability"], "focused-leaf");
    let decoded: AgentSessionRegistryIpcOperation =
        serde_json::from_value(value).expect("decode registration operation");
    assert_eq!(decoded, operation);
}

#[test]
fn control_plane_delegation_ipc_preserves_event_generation_and_child_capability() {
    let operation = AgentSessionRegistryIpcOperation::AdmitControlPlaneDelegation {
        proposal: SessionControlPlaneDelegationProposal {
            event_id: "event-1".to_owned(),
            project_id: "project-1".to_owned(),
            root_session_id: "root-1".to_owned(),
            current_session_id: "root-1".to_owned(),
            proposed_child_session_id: "asp-explorer-1".to_owned(),
            proposed_child_resident_name: "asp_explorer".to_owned(),
            proposed_child_capability: AgentSessionDelegationCapability::FocusedLeaf,
            expected_generation: 7,
            evidence_refs: vec!["codex-v2:verified-parent".to_owned()],
            observed_at_ms: 42,
        },
    };
    let value = serde_json::to_value(&operation).expect("encode delegation operation");
    assert_eq!(value["kind"], "admit-control-plane-delegation");
    assert_eq!(value["proposal"]["expectedGeneration"], 7);
    assert_eq!(value["proposal"]["proposedChildCapability"], "focused-leaf");
    let decoded: AgentSessionRegistryIpcOperation =
        serde_json::from_value(value).expect("decode delegation operation");
    assert_eq!(decoded, operation);
}
