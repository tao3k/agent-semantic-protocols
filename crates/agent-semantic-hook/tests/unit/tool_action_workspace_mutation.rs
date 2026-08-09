use super::workspace_mutation_paths;

#[test]
fn apply_patch_projects_sorted_deduplicated_workspace_paths() {
    // The Hook mutation projection is parser-owned and independent of policy matching.
    let input = serde_json::json!({
        "patch": "*** Begin Patch\n*** Update File: crates/agent-semantic-protocol/src/command/provider_dispatch.rs\n*** Update File: crates/agent-semantic-protocol/src/command/hook_runtime.rs\n*** Update File: crates/agent-semantic-protocol/src/command/provider_dispatch.rs\n*** End Patch"
    });

    assert_eq!(
        workspace_mutation_paths("apply_patch", &input),
        vec![
            "crates/agent-semantic-protocol/src/command/hook_runtime.rs".to_owned(),
            "crates/agent-semantic-protocol/src/command/provider_dispatch.rs".to_owned(),
        ]
    );
}

#[test]
fn non_mutating_tool_action_has_no_workspace_mutation_projection() {
    assert!(
        workspace_mutation_paths(
            "view_image",
            &serde_json::json!({ "path": "/tmp/image.png" })
        )
        .is_empty()
    );
}
