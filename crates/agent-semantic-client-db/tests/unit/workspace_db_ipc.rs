use tempfile::TempDir;

use agent_semantic_client_db::WorkspaceDbRegistry;
use agent_semantic_client_db::workspace_db_ipc::WorkspaceDbIpcOperation;

#[path = "workspace_db_ipc_generation_contract.rs"]
mod workspace_db_ipc_generation_contract;

#[test]
fn runtime_owner_tombstone_has_one_typed_v1_wire_shape() {
    let value = serde_json::to_value(WorkspaceDbIpcOperation::TombstoneRuntimeOwner {
        project_root: "/workspace".to_owned(),
        owner_path: "src/previous.rs".to_owned(),
    })
    .expect("encode runtime owner tombstone");
    assert_eq!(
        value,
        serde_json::json!({
            "kind": "tombstone-runtime-owner",
            "projectRoot": "/workspace",
            "ownerPath": "src/previous.rs"
        })
    );
}

#[test]
fn runtime_owner_relocation_has_one_atomic_v1_wire_shape() {
    let value = serde_json::to_value(WorkspaceDbIpcOperation::RelocateRuntimeOwner {
        project_root: "/workspace".to_owned(),
        previous_owner_path: "src/previous.rs".to_owned(),
        owner: agent_semantic_client_db::runtime_server_workspace::WorkspaceOwnerSnapshot {
            owner_path: "src/current.rs".to_owned(),
            content_digest:
                "blake3-256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
                    .to_owned(),
            bytes: b"fn current() {}\n".to_vec(),
            selectors: Vec::new(),
        },
    })
    .expect("encode runtime owner relocation");
    assert_eq!(value["kind"], "relocate-runtime-owner");
    assert_eq!(value["projectRoot"], "/workspace");
    assert_eq!(value["previousOwnerPath"], "src/previous.rs");
    assert_eq!(value["owner"]["ownerPath"], "src/current.rs");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn runtime_server_registry_rejects_non_gix_root_without_project_shell() {
    let fixture = TempDir::new().expect("create non-Gix registry fixture");
    let state_home = fixture.path().join("state");
    let project_root = fixture.path().join("ordinary-files");
    std::fs::create_dir_all(&project_root).expect("create ordinary file root");
    let registry = WorkspaceDbRegistry::with_state_home(&state_home);

    let Err(error) = registry.bootstrap_workspace(&project_root).await else {
        panic!("Runtime Server must reject a root not admitted by Gix");
    };

    assert!(
        error.contains("refusing to materialize") && error.contains("ephemeral"),
        "non-Gix temporary roots must fail closed with a stable materialization reason: {error}"
    );
    assert!(
        !state_home.join("projects/by-id").exists(),
        "rejected Tokio admission must create no State Core project shell"
    );
}
