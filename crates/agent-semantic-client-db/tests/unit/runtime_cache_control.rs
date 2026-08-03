use agent_semantic_client_db::workspace_db_ipc::{
    RuntimeCacheControlRequest, RuntimeCacheInvalidationScope, WorkspaceDbIpcOperation,
};

#[test]
fn cache_status_is_a_typed_runtime_server_operation() {
    let operation: WorkspaceDbIpcOperation = serde_json::from_value(serde_json::json!({
        "kind": "cache-control",
        "request": {
            "action": "status",
            "projectRoot": "/workspace/root"
        }
    }))
    .expect("cache status should decode through the workspace IPC contract");

    assert_eq!(
        operation,
        WorkspaceDbIpcOperation::CacheControl {
            request: RuntimeCacheControlRequest::Status {
                project_root: "/workspace/root".to_owned(),
            },
        }
    );
}

#[test]
fn cache_invalidation_requires_a_scoped_server_mutation() {
    let operation: WorkspaceDbIpcOperation = serde_json::from_value(serde_json::json!({
        "kind": "cache-control",
        "request": {
            "action": "invalidate",
            "projectRoot": "/workspace/root",
            "mutationId": "session-root/cache-invalidate-1",
            "scope": "syntax-rows"
        }
    }))
    .expect("cache invalidation should decode through the workspace IPC contract");

    assert_eq!(
        operation,
        WorkspaceDbIpcOperation::CacheControl {
            request: RuntimeCacheControlRequest::Invalidate {
                project_root: "/workspace/root".to_owned(),
                mutation_id: "session-root/cache-invalidate-1".to_owned(),
                scope: RuntimeCacheInvalidationScope::SyntaxRows,
            },
        }
    );
}

#[test]
fn cache_rebuild_rejects_an_empty_mutation_identity() {
    let error = serde_json::from_value::<WorkspaceDbIpcOperation>(serde_json::json!({
        "kind": "cache-control",
        "request": {
            "action": "rebuild-source-index",
            "projectRoot": "/workspace/root",
            "mutationId": ""
        }
    }))
    .expect_err("an empty mutation identity must fail closed");

    assert!(error.to_string().contains("mutation"));
}

#[test]
fn cache_request_rejects_cli_database_authority() {
    let error = serde_json::from_value::<WorkspaceDbIpcOperation>(serde_json::json!({
        "kind": "cache-control",
        "request": {
            "action": "status",
            "projectRoot": "/workspace/root",
            "databaseOwner": "cli"
        }
    }))
    .expect_err("cache authority is not a client-supplied field");

    assert!(error.to_string().contains("databaseOwner"));
}
