use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};

use super::{activation_repair_project_root, hook_workspace_candidate};

static NEXT_FIXTURE: AtomicU64 = AtomicU64::new(1);

#[test]
fn structured_tool_workdir_overrides_task_cwd() {
    let payload = serde_json::json!({
        "cwd": "/workspace/root",
        "tool_input": {
            "workdir": "/workspace/root/nested-provider"
        }
    });

    assert_eq!(
        hook_workspace_candidate(&payload, Path::new("/fallback")),
        Path::new("/workspace/root/nested-provider")
    );
}

#[test]
fn task_cwd_is_used_when_tool_workdir_is_absent() {
    let payload = serde_json::json!({
        "cwd": "/workspace/root",
        "tool_input": {}
    });

    assert_eq!(
        hook_workspace_candidate(&payload, Path::new("/fallback")),
        Path::new("/workspace/root")
    );
}

#[test]
fn project_root_is_the_final_fallback() {
    assert_eq!(
        hook_workspace_candidate(&serde_json::json!({}), Path::new("/fallback")),
        Path::new("/fallback")
    );
}

#[test]
fn activation_state_owner_controls_repair_when_process_cwd_differs() {
    let fixture = activation_fixture("owner");
    let project_root = fixture.join("project");
    let workspace_dir = fixture.join("state/workspaces/workspace-test");
    let activation_path = workspace_dir.join("hooks/state/activation.json");
    std::fs::create_dir_all(activation_path.parent().expect("activation parent"))
        .expect("create activation state");
    std::fs::create_dir_all(&project_root).expect("create project root");
    let project_root = std::fs::canonicalize(project_root).expect("canonical project root");
    std::fs::write(
        workspace_dir.join("workspace.json"),
        serde_json::json!({"root": project_root})
            .to_string()
            .as_bytes(),
    )
    .expect("write workspace identity");

    assert_eq!(
        activation_repair_project_root(&serde_json::json!({"cwd": project_root}), &activation_path)
            .expect("resolve canonical activation repair owner"),
        project_root
    );
    std::fs::remove_dir_all(fixture).expect("remove activation fixture");
}

#[test]
fn activation_repair_rejects_cross_identity_payload() {
    let fixture = activation_fixture("mismatch");
    let project_root = fixture.join("project");
    let other_root = fixture.join("other");
    let workspace_dir = fixture.join("state/workspaces/workspace-test");
    let activation_path = workspace_dir.join("hooks/state/activation.json");
    std::fs::create_dir_all(activation_path.parent().expect("activation parent"))
        .expect("create activation state");
    std::fs::create_dir_all(&project_root).expect("create project root");
    std::fs::create_dir_all(&other_root).expect("create other root");
    let project_root = std::fs::canonicalize(project_root).expect("canonical project root");
    let other_root = std::fs::canonicalize(other_root).expect("canonical other root");
    std::fs::write(
        workspace_dir.join("workspace.json"),
        serde_json::json!({"root": project_root})
            .to_string()
            .as_bytes(),
    )
    .expect("write workspace identity");

    let error =
        activation_repair_project_root(&serde_json::json!({"cwd": other_root}), &activation_path)
            .expect_err("cross-identity repair must fail closed");
    assert!(error.contains("identity/state mismatch"), "{error}");
    std::fs::remove_dir_all(fixture).expect("remove activation fixture");
}

fn activation_fixture(name: &str) -> std::path::PathBuf {
    let fixture = std::env::temp_dir().join(format!(
        "asp-hook-activation-repair-{name}-{}-{}",
        std::process::id(),
        NEXT_FIXTURE.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&fixture).expect("create activation fixture");
    fixture
}
