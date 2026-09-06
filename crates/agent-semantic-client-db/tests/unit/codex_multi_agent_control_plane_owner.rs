// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-only

use agent_semantic_client_db::codex_multi_agent_control_plane_owner::CodexMultiAgentControlPlaneOwner;
use agent_semantic_client_db::workspace_db_ipc::WorkspaceDbIpcOperation;
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
use agent_semantic_context_product::codex_multi_agent_v2_control_plane::CodexMultiAgentV2ControlPlaneProjection;

fn server(health: ServerHealth) -> WorkspaceServerProjection {
    WorkspaceServerProjection {
        workspace_identity: "workspace-example".to_owned(),
        health,
    }
}

fn projection(generation: u64) -> CodexMultiAgentV2ControlPlaneProjection {
    let workspace_server = server(ServerHealth::Ready);
    let lifecycle = AgentSessionLifecycleProjection::new(
        workspace_server.clone(),
        SessionLifecycleProjection {
            session_id: Some("child-session".to_owned()),
            generation: Some(1),
            phase: SessionPhase::Active,
        },
        HostBindingProjection {
            generation: Some(1),
            phase: BindingPhase::Fresh,
            child_session_id: Some("child-session".to_owned()),
            canonical_message_target: Some("agent://child-session".to_owned()),
            path_observed: Some(true),
        },
        DispatchLifecycleProjection {
            generation: Some(1),
            phase: DispatchPhase::Idle,
        },
    );
    CodexMultiAgentV2ControlPlaneProjection::new(
        CodexControlPlaneMaterialization {
            generation,
            source_digest: Some(format!("blake3-256:control-plane-{generation}")),
            evidence_refs: vec![format!("codex-receipt://host-tree-{generation}")],
            freshness: CodexControlPlaneFreshness::Current,
        },
        workspace_server,
        "root-session".to_owned(),
        vec![CodexAgentNodeProjection {
            root_session_id: "root-session".to_owned(),
            parent_session_id: Some("root-session".to_owned()),
            resident_name: "asp-worker".to_owned(),
            role: "worker".to_owned(),
            configured_agent_type: None,
            lifecycle,
        }],
        Vec::new(),
        Vec::new(),
    )
    .expect("valid Codex control-plane projection")
}

#[test]
fn runtime_server_ipc_exposes_typed_refresh_and_read_operations() {
    let publish = serde_json::to_value(
        WorkspaceDbIpcOperation::RefreshCodexMultiAgentControlPlane {
            project_id: "workspace-1".into(),
            root_session_id: "root-1".into(),
        },
    )
    .expect("serialize publish operation");
    assert_eq!(publish["kind"], "refresh-codex-multi-agent-control-plane");

    let read = serde_json::to_value(WorkspaceDbIpcOperation::ReadCodexMultiAgentControlPlane {
        root_session_id: "root-session".into(),
    })
    .expect("serialize read operation");
    assert_eq!(read["kind"], "read-codex-multi-agent-control-plane");
    assert_eq!(read["rootSessionId"], "root-session");
}

#[tokio::test]
async fn publication_is_idempotent_and_authorizes_from_the_current_snapshot() {
    let owner = CodexMultiAgentControlPlaneOwner::new();
    let first = owner
        .publish(projection(4))
        .await
        .expect("publish current snapshot");
    assert!(!first.idempotent);

    let replay = owner
        .publish(projection(4))
        .await
        .expect("idempotent replay");
    assert!(replay.idempotent);
    assert!(
        owner
            .downstream_dispatch_authorized("workspace-example", "root-session", "child-session")
            .await
    );
}

#[tokio::test]
async fn publication_rejects_stale_and_same_generation_conflicts() {
    let owner = CodexMultiAgentControlPlaneOwner::new();
    owner
        .publish(projection(4))
        .await
        .expect("publish current snapshot");

    let stale = owner.publish(projection(3)).await.expect_err("stale");
    assert!(stale.contains("stale Codex control-plane generation rejected"));

    let mut conflicting = projection(4);
    conflicting.materialization.source_digest = Some("blake3-256:conflict".to_owned());
    let conflict = owner.publish(conflicting).await.expect_err("conflict");
    assert!(conflict.contains("conflicting Codex control-plane materialization"));
}

#[tokio::test]
async fn workspace_server_change_marks_the_snapshot_stale_before_dispatch() {
    let owner = CodexMultiAgentControlPlaneOwner::new();
    owner
        .publish(projection(4))
        .await
        .expect("publish current snapshot");

    assert_eq!(
        owner
            .mark_workspace_server_observation(server(ServerHealth::Unavailable))
            .await,
        1
    );
    let snapshot = owner
        .read("workspace-example", "root-session")
        .await
        .expect("snapshot remains queryable");
    assert_eq!(
        snapshot.materialization.freshness,
        CodexControlPlaneFreshness::Stale
    );
    assert!(
        !owner
            .downstream_dispatch_authorized("workspace-example", "root-session", "child-session")
            .await
    );
}
