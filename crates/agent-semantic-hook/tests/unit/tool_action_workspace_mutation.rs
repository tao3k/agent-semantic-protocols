// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use super::workspace_mutation_paths;

#[test]
fn apply_patch_projects_sorted_deduplicated_workspace_paths() {
    // The Hook mutation projection is parser-owned and independent of policy matching.
    let input = serde_json::json!({
        "patch": "*** Begin Patch\n*** Update File: crates/agent-semantic-client/src/command/provider_dispatch.rs\n*** Update File: crates/agent-semantic-client/src/command/hook_runtime.rs\n*** Update File: crates/agent-semantic-client/src/command/provider_dispatch.rs\n*** End Patch"
    });

    assert_eq!(
        workspace_mutation_paths("apply_patch", &input),
        vec![
            "crates/agent-semantic-client/src/command/hook_runtime.rs".to_owned(),
            "crates/agent-semantic-client/src/command/provider_dispatch.rs".to_owned(),
        ]
    );
}

#[test]
fn codex_command_payload_projects_absolute_apply_patch_path() {
    let input = serde_json::json!({
        "command": "*** Begin Patch\n*** Update File: /workspace/crates/agent-semantic-hook/src/tool_action.rs\n@@\n+// lifecycle fixture\n*** End Patch"
    });

    assert_eq!(
        workspace_mutation_paths("apply_patch", &input),
        vec!["/workspace/crates/agent-semantic-hook/src/tool_action.rs".to_owned()]
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
