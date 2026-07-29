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
