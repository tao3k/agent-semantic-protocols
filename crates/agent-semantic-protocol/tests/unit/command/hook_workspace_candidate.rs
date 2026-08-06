use std::path::Path;

use super::hook_workspace_candidate;

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
fn explicit_asp_workspace_overrides_the_tool_workdir() {
    let payload = serde_json::json!({
        "cwd": "/workspace/root",
        "tool_input": {
            "workdir": "/workspace/root",
            "command": "/runtime/bin/asp rust query --selector rust://src/lib.rs#item/function/run --workspace crates/client-db --projection source"
        }
    });

    assert_eq!(
        hook_workspace_candidate(&payload, Path::new("/fallback")),
        Path::new("/workspace/root/crates/client-db")
    );
}

#[test]
fn explicit_absolute_asp_workspace_is_preserved() {
    let payload = serde_json::json!({
        "cwd": "/workspace/root",
        "tool_input": {
            "command": ["asp", "python", "query", "--workspace=/other/project"]
        }
    });

    assert_eq!(
        hook_workspace_candidate(&payload, Path::new("/fallback")),
        Path::new("/other/project")
    );
}

#[test]
fn another_commands_workspace_flag_does_not_change_runtime_admission_scope() {
    let payload = serde_json::json!({
        "cwd": "/workspace/root",
        "tool_input": {
            "command": "cargo test --workspace package"
        }
    });

    assert_eq!(
        hook_workspace_candidate(&payload, Path::new("/fallback")),
        Path::new("/workspace/root")
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
