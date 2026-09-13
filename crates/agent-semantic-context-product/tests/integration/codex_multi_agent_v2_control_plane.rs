// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use agent_semantic_context_product::agent_session_lifecycle::AgentSessionLifecycleProjection;
use agent_semantic_context_product::agent_session_lifecycle::BindingPhase;
use agent_semantic_context_product::agent_session_lifecycle::DispatchLifecycleProjection;
use agent_semantic_context_product::agent_session_lifecycle::DispatchPhase;
use agent_semantic_context_product::agent_session_lifecycle::HostBindingProjection;
use agent_semantic_context_product::agent_session_lifecycle::ServerHealth;
use agent_semantic_context_product::agent_session_lifecycle::SessionLifecycleProjection;
use agent_semantic_context_product::agent_session_lifecycle::SessionPhase;
use agent_semantic_context_product::agent_session_lifecycle::WorkspaceServerProjection;
use agent_semantic_context_product::codex_multi_agent_v2_control_plane::CodexAgentNodeProjection;
use agent_semantic_context_product::codex_multi_agent_v2_control_plane::CodexControlPlaneFreshness;
use agent_semantic_context_product::codex_multi_agent_v2_control_plane::CodexControlPlaneMaterialization;
use agent_semantic_context_product::codex_multi_agent_v2_control_plane::CodexDelegationEdgeProjection;
use agent_semantic_context_product::codex_multi_agent_v2_control_plane::CodexDelegationPhase;
use agent_semantic_context_product::codex_multi_agent_v2_control_plane::CodexMultiAgentV2ControlPlaneProjection;

fn server(health: ServerHealth) -> WorkspaceServerProjection {
    WorkspaceServerProjection {
        workspace_identity: "workspace-example".to_owned(),
        health,
    }
}

fn agent(
    session_id: &str,
    parent_session_id: Option<&str>,
    workspace_server: WorkspaceServerProjection,
) -> CodexAgentNodeProjection {
    CodexAgentNodeProjection {
        root_session_id: "root-session".to_owned(),
        parent_session_id: parent_session_id.map(str::to_owned),
        resident_name: format!("resident-{session_id}"),
        role: "worker".to_owned(),
        configured_agent_type: None,
        lifecycle: AgentSessionLifecycleProjection::new(
            workspace_server.clone(),
            SessionLifecycleProjection {
                session_id: Some(session_id.to_owned()),
                generation: Some(1),
                phase: SessionPhase::Active,
            },
            HostBindingProjection {
                generation: Some(1),
                phase: BindingPhase::Fresh,
                child_session_id: Some(session_id.to_owned()),
                canonical_message_target: Some(format!("agent://{session_id}")),
                path_observed: Some(true),
            },
            DispatchLifecycleProjection {
                generation: Some(1),
                phase: DispatchPhase::Idle,
            },
        ),
    }
}

fn projection() -> CodexMultiAgentV2ControlPlaneProjection {
    let workspace_server = server(ServerHealth::Ready);
    CodexMultiAgentV2ControlPlaneProjection::new(
        CodexControlPlaneMaterialization {
            generation: 4,
            source_digest: Some("blake3-256:control-plane-4".to_owned()),
            evidence_refs: vec!["codex-receipt://host-tree-4".to_owned()],
            freshness: CodexControlPlaneFreshness::Current,
        },
        workspace_server.clone(),
        "root-session".to_owned(),
        vec![agent(
            "child-session",
            Some("root-session"),
            workspace_server,
        )],
        Vec::new(),
        vec![CodexDelegationEdgeProjection {
            parent_session_id: "root-session".to_owned(),
            parent_turn_id: None,
            child_session_id: "child-session".to_owned(),
            child_generation: 1,
            phase: CodexDelegationPhase::Delivered,
            delivered_receipt_ref: Some("codex-receipt://delegation-1".to_owned()),
        }],
    )
    .expect("valid Codex control plane")
}

#[test]
fn current_snapshot_authorizes_only_a_durable_leaf() {
    let projection = projection();
    assert!(projection.downstream_dispatch_authorized("child-session"));
    assert!(!projection.downstream_dispatch_authorized("missing-session"));
}

#[test]
fn server_observation_change_makes_the_snapshot_stale() {
    let projection = projection().with_workspace_server(server(ServerHealth::Unavailable));
    assert_eq!(
        projection.materialization.freshness,
        CodexControlPlaneFreshness::Stale
    );
    assert!(!projection.downstream_dispatch_authorized("child-session"));
}

#[test]
fn delivered_delegation_requires_an_indexed_codex_receipt() {
    let mut projection = projection();
    projection.delegations[0].delivered_receipt_ref = None;
    assert!(
        projection
            .validate()
            .expect_err("missing receipt must fail")
            .contains("requires an indexed receipt reference")
    );
}

#[test]
fn stale_child_generation_cannot_enter_the_control_plane() {
    let mut projection = projection();
    projection.delegations[0].child_generation = 0;
    assert!(
        projection
            .validate()
            .expect_err("stale generation must fail")
            .contains("child generation is stale")
    );
}

#[test]
fn publication_admission_is_monotonic_and_idempotent() {
    let current = projection();
    assert_eq!(
        current
            .publication_admission(&current)
            .expect("identical replay"),
        agent_semantic_context_product::codex_multi_agent_v2_control_plane::CodexControlPlanePublicationAdmission::Idempotent
    );

    let mut advanced = projection();
    advanced.materialization.generation = 5;
    advanced.materialization.source_digest = Some("blake3-256:control-plane-5".to_owned());
    assert_eq!(
        current
            .publication_admission(&advanced)
            .expect("new generation"),
        agent_semantic_context_product::codex_multi_agent_v2_control_plane::CodexControlPlanePublicationAdmission::Advance
    );

    let mut stale = projection();
    stale.materialization.generation = 3;
    assert!(
        current
            .publication_admission(&stale)
            .expect_err("stale generation")
            .contains("stale Codex control-plane generation rejected")
    );
}

#[test]
fn same_generation_conflict_is_not_an_idempotent_replay() {
    let current = projection();
    let mut conflicting = projection();
    conflicting.materialization.source_digest = Some("blake3-256:conflict".to_owned());
    assert!(
        current
            .publication_admission(&conflicting)
            .expect_err("same-generation conflict")
            .contains("conflicting Codex control-plane materialization")
    );
}
