use super::direct_read_policy_project_root;

#[test]
fn direct_read_policy_root_does_not_require_executable_activation() {
    let root = tempfile::tempdir().expect("direct-read project root");
    let payload = serde_json::json!({
        "cwd": root.path(),
        "tool_name": "Read",
        "tool_input": {"file_path": "README.md"},
    });
    assert_eq!(
        direct_read_policy_project_root(&payload),
        Some(root.path().canonicalize().expect("canonical project root"))
    );
}

#[test]
fn command_execution_still_requires_runtime_activation() {
    let payload = serde_json::json!({
        "tool_name": "Bash",
        "tool_input": {"command": "cargo test"},
    });
    assert_eq!(direct_read_policy_project_root(&payload), None);
}
