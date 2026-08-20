use super::{collect_tool_actions, workspace_mutation_paths};
use serde_json::json;

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
            "cmd": "asp server status && ASP_NO_AGENT=1 shasum -a 256 target/release/asp .bin/asp && ASP_NO_AGENT=1 git diff --check"
        }),
    );
    assert_eq!(actions.len(), 3);
    assert_eq!(
        actions[0].command_tokens.as_deref().unwrap()[..3],
        ["asp", "server", "status"]
    );
    assert_eq!(
        actions[1].command_tokens.as_deref().unwrap()[0],
        "ASP_NO_AGENT=1"
    );
    assert_eq!(actions[1].command_tokens.as_deref().unwrap()[1], "shasum");
    assert_eq!(
        actions[2].command_tokens.as_deref().unwrap()[1..],
        ["git", "diff", "--check"]
    );
}
