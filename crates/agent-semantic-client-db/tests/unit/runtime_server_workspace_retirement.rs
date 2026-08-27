use super::RuntimeServerWorkspaceRegistry;
use crate::runtime_server_workspace::lease::WorkspaceResidentActivity;
use crate::runtime_server_workspace::{
    RESIDENT_WORKSPACE_RETIREMENT_RECEIPT_SCHEMA_ID, ResidentWorkspaceRetirementReason,
};
use std::sync::Arc;
use std::time::Duration;

#[test]
fn in_flight_request_prevents_retirement_admission_close() {
    let activity = Arc::new(WorkspaceResidentActivity::new());
    let request = activity
        .begin_request()
        .expect("resident request should enter before retirement");

    assert!(!activity.try_close_for_retirement(false, Duration::ZERO));
    drop(request);
    assert!(activity.try_close_for_retirement(false, Duration::ZERO));
}

#[test]
fn retirement_receipt_schema_keeps_v1_identity() {
    let schema: serde_json::Value = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../schemas/resident-workspace-retirement-receipt.v1.schema.json"
    )))
    .expect("retirement receipt schema JSON");

    assert_eq!(
        schema["properties"]["schemaId"]["const"],
        RESIDENT_WORKSPACE_RETIREMENT_RECEIPT_SCHEMA_ID
    );
    assert_eq!(schema["properties"]["schemaVersion"]["const"], "1");
    assert_eq!(schema["properties"]["idleTimeoutSeconds"]["minimum"], 3_600);
}

#[tokio::test]
async fn removed_workspace_is_drained_and_retired_once() {
    let state_root = tempfile::tempdir().expect("state root");
    let missing_workspace = state_root.path().join("removed-worktree");
    let registry =
        RuntimeServerWorkspaceRegistry::new(state_root.path().join("runtime")).expect("registry");

    registry
        .entry("workspace-removed", &missing_workspace)
        .await
        .expect("resident entry");
    assert_eq!(registry.workspace_count(), 1);

    let receipts = registry.retire_inactive().await.expect("retirement sweep");
    assert_eq!(receipts.len(), 1);
    assert_eq!(
        receipts[0].reason,
        ResidentWorkspaceRetirementReason::WorkspaceMissing
    );
    assert_eq!(registry.workspace_count(), 0);
    assert!(
        registry
            .retire_inactive()
            .await
            .expect("second sweep")
            .is_empty()
    );
}

#[tokio::test]
async fn existing_workspace_retires_only_after_one_hour_idle() {
    let state_root = tempfile::tempdir().expect("state root");
    let workspace = tempfile::tempdir().expect("workspace");
    let registry =
        RuntimeServerWorkspaceRegistry::new(state_root.path().join("runtime")).expect("registry");

    registry
        .entry("workspace-idle", workspace.path())
        .await
        .expect("resident entry");
    assert!(
        registry
            .retire_inactive()
            .await
            .expect("fresh sweep")
            .is_empty()
    );

    let resident = registry
        .entries
        .read()
        .get("workspace-idle")
        .cloned()
        .expect("resident activity");
    resident
        .activity
        .set_idle_for_test(Duration::from_secs(3_600));

    let receipts = registry.retire_inactive().await.expect("idle sweep");
    assert_eq!(receipts.len(), 1);
    assert_eq!(
        receipts[0].reason,
        ResidentWorkspaceRetirementReason::IdleTimeout
    );
    assert_eq!(registry.workspace_count(), 0);
}
