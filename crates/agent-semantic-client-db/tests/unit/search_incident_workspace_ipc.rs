use std::time::Duration;

use crate::{
    runtime_server_workspace::ExactProjectionKind,
    runtime_telemetry_bus::{RuntimeTelemetryBus, RuntimeTelemetryEvent},
    workspace_db_ipc::{
        WORKSPACE_DB_OWNER_REQUEST_SCHEMA_ID, WORKSPACE_DB_OWNER_SCHEMA_VERSION,
        WorkspaceDbIpcOperation, WorkspaceDbIpcRequest, WorkspaceDbIpcResult,
    },
};

use super::{record_workspace_ipc_terminal, workspace_ipc_terminal_context};

fn selector_request(workspace_identity: &str) -> WorkspaceDbIpcRequest {
    WorkspaceDbIpcRequest {
        schema_id: WORKSPACE_DB_OWNER_REQUEST_SCHEMA_ID.into(),
        schema_version: WORKSPACE_DB_OWNER_SCHEMA_VERSION.to_owned(),
        workspace_identity: workspace_identity.to_owned(),
        transport_contract_digest: "transport".to_owned(),
        owner_epoch: 1,
        binding_token: "binding".into(),
        request_id: format!("request-{workspace_identity}").into(),
        operation: WorkspaceDbIpcOperation::ReadRuntimeSelector {
            project_root: "/workspace".to_owned(),
            language_id: serde_json::from_str("\"rust\"").expect("typed language id"),
            projection_kind: ExactProjectionKind::Source,
            structural_selector: "rust://src/lib.rs#item/function/example".to_owned(),
        },
    }
}

#[tokio::test]
async fn failed_exact_requests_emit_typed_workspace_isolated_incidents() {
    let mut bus = RuntimeTelemetryBus::new();
    let failed = WorkspaceDbIpcResult::Failed {
        code: "selector-not-in-active-generation".to_owned(),
        message: "diagnostic body is not parsed".to_owned(),
    };
    for workspace in ["workspace-a", "workspace-b"] {
        let request = selector_request(workspace);
        let context = workspace_ipc_terminal_context(&request).expect("exact query context");
        let record = record_workspace_ipc_terminal(
            Some(&bus.sender),
            Some(context),
            &failed,
            Duration::from_micros(41),
        )
        .expect("terminal admission")
        .expect("active incident");
        assert_eq!(record.identity.workspace_identity, workspace);
        assert_eq!(record.identity.language_id, "rust");
    }

    let mut workspaces = std::collections::BTreeSet::new();
    for _ in 0..2 {
        let RuntimeTelemetryEvent::SearchIncident(event) =
            bus.receiver.recv().await.expect("terminal incident")
        else {
            panic!("expected search incident")
        };
        assert_eq!(event.observation.elapsed_micros, Some(41));
        assert_eq!(
            event.observation.identity.reason_kind,
            "selector-not-in-active-generation"
        );
        workspaces.insert(event.observation.identity.workspace_identity);
    }
    assert_eq!(
        workspaces,
        ["workspace-a", "workspace-b"].map(str::to_owned).into()
    );
}

#[tokio::test]
async fn successful_exact_requests_do_not_emit_incidents() {
    let mut bus = RuntimeTelemetryBus::new();
    let request = selector_request("workspace-success");
    let admitted = record_workspace_ipc_terminal(
        Some(&bus.sender),
        workspace_ipc_terminal_context(&request),
        &WorkspaceDbIpcResult::Healthy,
        Duration::from_micros(1),
    )
    .expect("successful terminal classification");
    assert!(admitted.is_none());
    assert!(bus.receiver.try_recv().is_err());
}
