use agent_semantic_protocol::agent_session_lifecycle_projection::{
    AgentSessionLifecycleProjection, BindingPhase, DispatchLifecycleProjection, DispatchPhase,
    HostBindingProjection, ServerHealth, SessionLifecycleProjection, SessionPhase,
    WorkspaceServerProjection,
};
use agent_semantic_protocol::codex_multi_agent_v2_control_plane::{
    CodexAgentNodeProjection, CodexControlPlaneFreshness, CodexControlPlaneMaterialization,
    CodexDelegationEdgeProjection, CodexDelegationPhase, CodexMultiAgentV2ControlPlaneProjection,
    CodexTurnNodeProjection, CodexTurnPhase,
};

fn server(health: ServerHealth) -> WorkspaceServerProjection {
    WorkspaceServerProjection {
        workspace_identity: "workspace-example".to_owned(),
        health,
    }
}

fn lifecycle(
    session_id: &str,
    generation: u64,
    workspace_server: WorkspaceServerProjection,
) -> AgentSessionLifecycleProjection {
    AgentSessionLifecycleProjection::new(
        workspace_server,
        SessionLifecycleProjection {
            session_id: Some(session_id.to_owned()),
            generation: Some(generation),
            phase: SessionPhase::Active,
        },
        HostBindingProjection {
            generation: Some(generation),
            phase: BindingPhase::Fresh,
            child_session_id: Some(session_id.to_owned()),
            canonical_message_target: Some(format!("agent://{session_id}")),
            termination_receipt_indexed: false,
            path_release_receipt_indexed: false,
        },
        DispatchLifecycleProjection {
            generation: Some(generation),
            phase: DispatchPhase::Idle,
        },
    )
}

fn root_agent(workspace_server: WorkspaceServerProjection) -> CodexAgentNodeProjection {
    CodexAgentNodeProjection {
        root_session_id: "root-session".to_owned(),
        parent_session_id: None,
        resident_name: "asp-main".to_owned(),
        role: "main".to_owned(),
        configured_agent_type: None,
        lifecycle: lifecycle("root-session", 1, workspace_server),
    }
}

fn child_agent(workspace_server: WorkspaceServerProjection) -> CodexAgentNodeProjection {
    CodexAgentNodeProjection {
        root_session_id: "root-session".to_owned(),
        parent_session_id: Some("root-session".to_owned()),
        resident_name: "asp-testing".to_owned(),
        role: "testing".to_owned(),
        configured_agent_type: Some("asp_testing".to_owned()),
        lifecycle: lifecycle("child-session", 1, workspace_server),
    }
}

fn valid_projection() -> CodexMultiAgentV2ControlPlaneProjection {
    let workspace_server = server(ServerHealth::Ready);
    CodexMultiAgentV2ControlPlaneProjection::new(
        CodexControlPlaneMaterialization {
            generation: 4,
            source_digest: Some("blake3-256:control-plane-example".to_owned()),
            evidence_refs: vec!["codex-receipt://host-tree-4".to_owned()],
            freshness: CodexControlPlaneFreshness::Current,
        },
        workspace_server.clone(),
        "root-session".to_owned(),
        vec![
            root_agent(workspace_server.clone()),
            child_agent(workspace_server),
        ],
        vec![CodexTurnNodeProjection {
            turn_id: "turn-root".to_owned(),
            session_id: "root-session".to_owned(),
            generation: Some(1),
            phase: CodexTurnPhase::Running,
        }],
        vec![CodexDelegationEdgeProjection {
            parent_session_id: "root-session".to_owned(),
            parent_turn_id: Some("turn-root".to_owned()),
            child_session_id: "child-session".to_owned(),
            child_generation: 1,
            phase: CodexDelegationPhase::Delivered,
            delivered_receipt_ref: Some("codex-receipt://delegation-1".to_owned()),
        }],
    )
    .expect("valid Codex control-plane graph")
}

#[test]
fn valid_codex_host_tree_is_admitted() {
    let projection = valid_projection();
    assert_eq!(projection.agents.len(), 2);
    assert_eq!(projection.turns.len(), 1);
    assert_eq!(projection.delegations.len(), 1);
}

#[test]
fn duplicate_agent_session_is_rejected() {
    let workspace_server = server(ServerHealth::Ready);
    let duplicate = root_agent(workspace_server.clone());
    let error = CodexMultiAgentV2ControlPlaneProjection::new(
        CodexControlPlaneMaterialization {
            generation: 1,
            source_digest: Some("blake3-256:duplicate".to_owned()),
            evidence_refs: vec!["codex-receipt://duplicate".to_owned()],
            freshness: CodexControlPlaneFreshness::Current,
        },
        workspace_server.clone(),
        "root-session".to_owned(),
        vec![root_agent(workspace_server), duplicate],
        vec![],
        vec![],
    )
    .expect_err("duplicate session must fail");
    assert!(error.contains("duplicate Codex agent session"));
}

#[test]
fn missing_parent_is_rejected() {
    let workspace_server = server(ServerHealth::Ready);
    let mut child = child_agent(workspace_server.clone());
    child.parent_session_id = Some("missing-parent".to_owned());
    let error = CodexMultiAgentV2ControlPlaneProjection::new(
        CodexControlPlaneMaterialization {
            generation: 1,
            source_digest: Some("blake3-256:missing-parent".to_owned()),
            evidence_refs: vec!["codex-receipt://missing-parent".to_owned()],
            freshness: CodexControlPlaneFreshness::Current,
        },
        workspace_server.clone(),
        "root-session".to_owned(),
        vec![root_agent(workspace_server), child],
        vec![],
        vec![],
    )
    .expect_err("missing parent must fail");
    assert!(error.contains("missing parent"));
}

#[test]
fn stale_child_generation_is_rejected() {
    let mut projection = valid_projection();
    projection.delegations[0].child_generation = 0;
    assert!(
        projection
            .validate()
            .expect_err("stale child generation must fail")
            .contains("stale")
    );
}

#[test]
fn delivered_phase_without_codex_receipt_is_rejected() {
    let mut projection = valid_projection();
    projection.delegations[0].delivered_receipt_ref = None;
    assert!(
        projection
            .validate()
            .expect_err("delivery without receipt must fail")
            .contains("indexed receipt")
    );
}

#[test]
fn workspace_server_restart_preserves_codex_graph_identity() {
    let projection = valid_projection();
    let agent_ids = projection
        .agents
        .iter()
        .map(|agent| agent.lifecycle.session.session_id.clone())
        .collect::<Vec<_>>();
    let restarted = projection.with_workspace_server(server(ServerHealth::Repairing));
    assert_eq!(
        restarted
            .agents
            .iter()
            .map(|agent| agent.lifecycle.session.session_id.clone())
            .collect::<Vec<_>>(),
        agent_ids
    );
    assert_eq!(restarted.turns.len(), 1);
    assert_eq!(restarted.delegations.len(), 1);
    assert_eq!(
        restarted.materialization.freshness,
        CodexControlPlaneFreshness::Stale
    );
    restarted.validate().expect("restarted control plane");
}

#[test]
fn stale_daemon_snapshot_cannot_authorize_downstream_dispatch() {
    let mut projection = valid_projection();
    assert!(projection.downstream_dispatch_authorized("child-session"));
    projection.materialization.freshness = CodexControlPlaneFreshness::Stale;
    assert!(!projection.downstream_dispatch_authorized("child-session"));
}

#[test]
fn serialized_contract_is_codex_specific_and_round_trips() {
    let projection = valid_projection();
    let value = serde_json::to_value(&projection).expect("serialize control plane");
    assert_eq!(
        value["schemaId"],
        "agent.semantic-protocols.codex-multi-agent-v2-control-plane-projection"
    );
    let round_trip: CodexMultiAgentV2ControlPlaneProjection =
        serde_json::from_value(value).expect("deserialize control plane");
    assert_eq!(round_trip, projection);
}
