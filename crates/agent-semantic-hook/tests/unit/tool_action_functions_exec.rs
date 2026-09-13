// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use super::OperationIntent;
use super::collect_tool_actions;
use super::workspace_mutation_paths;
use serde_json::json;

#[test]
fn canonical_host_envelopes_project_parser_owned_operation_intents() {
    let cases = [
        (
            "fs/readFile",
            json!({ "path": "docs/plan.org" }),
            OperationIntent::DirectRead,
            "docs/plan.org",
        ),
        (
            "apply_patch",
            json!({
                "patch": "*** Begin Patch\n*** Update File: src/lib.rs\n@@\n-old\n+new\n*** End Patch"
            }),
            OperationIntent::ApplyPatch,
            "src/lib.rs",
        ),
        (
            "Bash",
            json!({ "command": "printf ok" }),
            OperationIntent::ShellCommand,
            "",
        ),
        (
            "Grep",
            json!({ "pattern": "owner", "path": "src" }),
            OperationIntent::FileSearch,
            "src",
        ),
        (
            "fs/readDirectory",
            json!({ "path": "docs" }),
            OperationIntent::DirectoryRead,
            "docs",
        ),
        (
            "mcp__filesystem__read_file",
            json!({ "uri": "README.md" }),
            OperationIntent::DirectRead,
            "README.md",
        ),
    ];

    for (tool_name, payload, expected_operation, expected_path) in cases {
        let actions = collect_tool_actions(tool_name, &payload);
        let action = actions
            .iter()
            .find(|action| action.operation == expected_operation)
            .unwrap_or_else(|| panic!("missing {expected_operation:?} for {tool_name}"));
        if !expected_path.is_empty() {
            assert!(
                action.paths.iter().any(|path| path == expected_path),
                "missing {expected_path} for {tool_name}: {:?}",
                action.paths
            );
        }
        if tool_name == "mcp__filesystem__read_file" {
            let agent_action = action.derive_agent_action();
            assert_eq!(
                agent_action.host.action,
                crate::action_ir::HostInvocationKind::Unknown
            );
            assert!(
                agent_action
                    .capabilities
                    .iter()
                    .all(|capability| capability.action != crate::action_ir::AgentActionKind::Read),
                "payload shape alone must not invent the native Host matcher"
            );
        }
    }
}

#[test]
fn nested_host_envelopes_preserve_each_semantic_operation() {
    let payload = json!({
        "tool_uses": [
            {
                "recipient_name": "fs/readFile",
                "parameters": { "path": "README.md" }
            },
            {
                "recipient_name": "apply_patch",
                "parameters": {
                    "patch": "*** Begin Patch\n*** Update File: src/lib.rs\n@@\n-old\n+new\n*** End Patch"
                }
            }
        ]
    });

    let actions = collect_tool_actions("multi_tool_use.parallel", &payload);
    assert!(actions.iter().any(|action| {
        action.operation == OperationIntent::DirectRead
            && action.paths.iter().any(|path| path == "README.md")
    }));
    assert!(actions.iter().any(|action| {
        action.operation == OperationIntent::ApplyPatch
            && action.paths.iter().any(|path| path == "src/lib.rs")
    }));
}

#[test]
fn functions_exec_apply_patch_binding_projects_workspace_mutation_paths() {
    let payload = json!({
        "code": r#"const patch = "*** Begin Patch\n*** Update File: crates/demo/src/lib.rs\n@@\n-old\n+new\n*** End Patch";
    const receipt = await tools.apply_patch(patch);"#
    });

    assert_eq!(
        workspace_mutation_paths("functions.exec", &payload),
        ["crates/demo/src/lib.rs"]
    );
}

#[test]
fn functions_exec_apply_patch_literal_projects_workspace_mutation_paths() {
    let payload = json!({
        "code": r#"await tools.apply_patch("*** Begin Patch\n*** Add File: src/new.rs\n+new\n*** End Patch");"#
    });

    assert_eq!(
        workspace_mutation_paths("functions.exec", &payload),
        ["src/new.rs"]
    );
}

#[test]
fn functions_exec_interpolated_patch_is_not_guessed_as_a_mutation() {
    let payload = json!({
        "code": r#"const patch = `*** Begin Patch\n*** Add File: ${path}\n+new\n*** End Patch`;
    await tools.apply_patch(patch);"#
    });

    assert!(workspace_mutation_paths("functions.exec", &payload).is_empty());
}

#[test]
fn functions_exec_freeform_input_projects_apply_patch_mutation_paths() {
    let payload = json!(
        r#"await tools.apply_patch("*** Begin Patch\n*** Add File: src/freeform.rs\n+new\n*** End Patch");"#
    );

    assert_eq!(
        workspace_mutation_paths("functions.exec", &payload),
        ["src/freeform.rs"]
    );
}

#[test]
fn compound_shell_commands_preserve_one_normalized_action_per_stage() {
    let actions = collect_tool_actions(
        "functions.exec_command",
        &json!({
            "cmd": "asp server status && CHECK_MODE=1 shasum -a 256 target/release/asp .bin/asp && CHECK_MODE=1 git diff --check"
        }),
    );
    assert_eq!(actions.len(), 3);
    assert_eq!(
        actions[0].command_tokens.as_deref().unwrap()[..3],
        ["asp", "server", "status"]
    );
    assert_eq!(
        actions[1].command_tokens.as_deref().unwrap()[0],
        "CHECK_MODE=1"
    );
    assert_eq!(actions[1].command_tokens.as_deref().unwrap()[1], "shasum");
    assert_eq!(
        actions[2].command_tokens.as_deref().unwrap()[1..],
        ["git", "diff", "--check"]
    );
}
