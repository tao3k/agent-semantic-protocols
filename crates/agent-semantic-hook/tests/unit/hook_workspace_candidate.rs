use std::path::{Path, PathBuf};

use agent_semantic_hook::hook_workspace_candidate;
use serde_json::json;

#[test]
fn resolves_payload_cwd_and_relative_workdir_lexically() {
    let payload = json!({
        "cwd": "/workspace/repo",
        "tool_input": { "workdir": "crates/../languages" }
    });
    assert_eq!(
        hook_workspace_candidate(&payload, Path::new("/fallback")),
        PathBuf::from("/workspace/repo/languages")
    );
}

#[test]
fn explicit_asp_workspace_overrides_command_root() {
    let payload = json!({
        "cwd": "/workspace/repo",
        "tool_input": {
            "command": "direnv exec . asp rust query owner --workspace ../target"
        }
    });
    assert_eq!(
        hook_workspace_candidate(&payload, Path::new("/fallback")),
        PathBuf::from("/workspace/target")
    );
}

#[test]
fn nested_command_arrays_are_projected_without_protocol_knowledge() {
    let payload = json!({
        "cwd": "/workspace/repo",
        "tool_input": {
            "invocation": { "cmd": ["asp", "rust", "query", "--workspace=member"] }
        }
    });
    assert_eq!(
        hook_workspace_candidate(&payload, Path::new("/fallback")),
        PathBuf::from("/workspace/repo/member")
    );
}
