// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-only

use agent_semantic_client_db::workspace_db_ipc::RuntimeCacheControlRequest;
use agent_semantic_client_db::workspace_db_ipc::RuntimeCacheInvalidationScope;
use agent_semantic_client_db::workspace_db_ipc::WorkspaceDbIpcOperation;

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
fn runtime_merkle_owner_read_is_a_typed_runtime_data_plane_operation() {
    let operation: WorkspaceDbIpcOperation = serde_json::from_value(serde_json::json!({
        "kind": "read-runtime-merkle-owner",
        "request": {
            "schemaId": agent_semantic_client_db::workspace_db_ipc::RUNTIME_MERKLE_OWNER_READ_REQUEST_SCHEMA_ID,
            "schemaVersion": "1",
            "projectRoot": "/workspace/root",
            "ownerPath": "src/lib.rs"
        }
    }))
    .expect("decode typed Merkle owner request");

    assert_eq!(
        operation,
        WorkspaceDbIpcOperation::ReadRuntimeMerkleOwner {
            request: agent_semantic_client_db::workspace_db_ipc::RuntimeMerkleOwnerReadRequest::new(
                "/workspace/root",
                "src/lib.rs",
            ),
        }
    );
}

#[test]
fn runtime_merkle_owner_read_rejects_non_normalized_owner_paths() {
    let request = agent_semantic_client_db::workspace_db_ipc::RuntimeMerkleOwnerReadRequest::new(
        "/workspace/root",
        "src/../secret.rs",
    );
    assert_eq!(
        request
            .validate()
            .expect_err("parent path must fail closed"),
        "runtime Merkle owner path must be normalized and relative"
    );
}

#[test]
fn cache_owner_delta_decodes_the_schema_owned_fail_closed_policy() {
    let operation: WorkspaceDbIpcOperation = serde_json::from_value(serde_json::json!({
        "kind": "cache-control",
        "request": {
            "action": "apply-owner-delta",
            "projectRoot": "/workspace/root",
            "mutationId": "session-root/cache-owner-delta-1",
            "changedPaths": ["src/lib.rs"],
            "removedPaths": [],
            "fallbackPolicy": "full-generation"
        }
    }))
    .expect("cache owner delta should decode through the workspace IPC contract");

    assert_eq!(
        operation,
        WorkspaceDbIpcOperation::CacheControl {
            request: RuntimeCacheControlRequest::ApplyOwnerDelta {
                project_root: "/workspace/root".to_owned(),
                mutation_id: "session-root/cache-owner-delta-1".to_owned(),
                changed_paths: vec!["src/lib.rs".to_owned()],
                removed_paths: Vec::new(),
                fallback_policy:
                    agent_semantic_client_db::workspace_db_ipc::RuntimeCacheOwnerDeltaFallbackPolicy::FullGeneration,
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
