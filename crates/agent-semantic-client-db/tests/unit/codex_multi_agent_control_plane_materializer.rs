use super::materialize_codex_multi_agent_control_plane;
use crate::agent_session_registry::AgentSessionRecord;
use crate::workspace_db_ipc::WorkspaceDbIpcOperation;

fn registry_record(
    session_id: &str,
    parent_session_id: Option<&str>,
    updated_at: i64,
) -> AgentSessionRecord {
    serde_json::from_value(serde_json::json!({
        "projectId": "workspace-1",
        "rootSessionId": "root-1",
        "sessionId": session_id,
        "physicalGeneration": 1,
        "parentSessionId": parent_session_id,
        "name": if parent_session_id.is_none() { "root" } else { "child" },
        "role": if parent_session_id.is_none() { "root" } else { "worker" },
        "status": "active",
        "createdAt": 1,
        "updatedAt": updated_at,
        "metadataJson": "{}"
    }))
    .expect("registry fixture must deserialize")
}

#[test]
fn registry_materialization_requires_scoped_root_task_evidence() {
    let error = materialize_codex_multi_agent_control_plane(
        "workspace-1",
        "workspace-1",
        "root-1",
        Vec::new(),
        None,
    )
    .expect_err("a root id alone must not synthesize a Codex root task");

    assert!(error.contains("non-empty scoped AgentSession Registry snapshot"));
}

#[test]
fn registry_materialization_rejects_root_task_impersonation() {
    let error = materialize_codex_multi_agent_control_plane(
        "workspace-1",
        "workspace-1",
        "root-1",
        vec![registry_record("root-1", None, 1)],
        None,
    )
    .expect_err("root task identity must not acquire invented AgentSession lifecycle facts");

    assert!(error.contains("must not be encoded as an AgentSession Registry row"));
}

#[test]
fn registry_materialization_rejects_cross_workspace_scope() {
    let error = materialize_codex_multi_agent_control_plane(
        "workspace-1",
        "workspace-2",
        "root-1",
        vec![registry_record("child-1", Some("root-1"), 1)],
        None,
    )
    .expect_err("registry project scope must not cross the Runtime Server workspace");

    assert!(error.contains("project scope must match"));
}

#[test]
fn refresh_operation_has_a_typed_v1_wire_shape() {
    let value = serde_json::to_value(
        WorkspaceDbIpcOperation::RefreshCodexMultiAgentControlPlane {
            project_id: "workspace-1".to_owned(),
            root_session_id: "root-1".to_owned(),
        },
    )
    .expect("refresh operation must serialize");

    assert_eq!(value["kind"], "refresh-codex-multi-agent-control-plane");
    assert_eq!(value["projectId"], "workspace-1");
    assert_eq!(value["rootSessionId"], "root-1");
}

#[test]
fn registry_materialization_does_not_invent_turns_or_delegation_receipts() {
    let projection = materialize_codex_multi_agent_control_plane(
        "workspace-1",
        "workspace-1",
        "root-1",
        vec![registry_record("child-1", Some("root-1"), 1)],
        None,
    )
    .expect("durable registry graph must materialize");

    assert_eq!(projection.root_task.session_id, "root-1");
    assert!(projection.root_task.evidence_ref.is_some());
    assert_eq!(projection.agents.len(), 1);
    assert!(projection.turns.is_empty());
    assert!(projection.delegations.is_empty());
    assert!(
        projection
            .agents
            .iter()
            .all(|agent| !agent.lifecycle.durable_dispatch_authorized)
    );
}

#[test]
fn unchanged_registry_snapshot_is_idempotent_but_changed_evidence_advances_generation() {
    let records = vec![registry_record("child-1", Some("root-1"), 1)];
    let first = materialize_codex_multi_agent_control_plane(
        "workspace-1",
        "workspace-1",
        "root-1",
        records.clone(),
        None,
    )
    .expect("initial registry graph must materialize");
    let unchanged = materialize_codex_multi_agent_control_plane(
        "workspace-1",
        "workspace-1",
        "root-1",
        records,
        Some(&first),
    )
    .expect("unchanged registry graph must be idempotent");
    let changed = materialize_codex_multi_agent_control_plane(
        "workspace-1",
        "workspace-1",
        "root-1",
        vec![registry_record("child-1", Some("root-1"), 2)],
        Some(&unchanged),
    )
    .expect("changed registry graph must advance");

    assert_eq!(unchanged, first);
    assert_eq!(changed.materialization.generation, 2);
    assert_ne!(
        changed.materialization.source_digest,
        first.materialization.source_digest
    );
}
