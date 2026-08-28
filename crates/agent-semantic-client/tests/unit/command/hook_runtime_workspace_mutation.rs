use super::post_tool_workspace_mutation;
use serde_json::json;

#[test]
fn functions_exec_post_tool_projects_typed_workspace_mutation() {
    let payload = json!({
        "tool_name": "functions.exec",
        "tool_use_id": "exec-mutation-1",
        "tool_input": {
            "code": r#"const patch = "*** Begin Patch\n*** Update File: crates/demo/src/lib.rs\n@@\n-old\n+new\n*** End Patch";
    await tools.apply_patch(patch);"#
        }
    });

    let mutation = post_tool_workspace_mutation("post-tool", &payload)
        .expect("project post-tool mutation")
        .expect("typed mutation");
    assert_eq!(mutation.mutation_id, "exec-mutation-1");
    assert_eq!(mutation.changed_paths, ["crates/demo/src/lib.rs"]);
}

#[test]
fn non_mutating_post_tool_has_no_generation_effect() {
    let payload = json!({
        "tool_name": "functions.exec",
        "tool_use_id": "exec-read-1",
        "tool_input": { "code": "text(true);" }
    });
    assert!(
        post_tool_workspace_mutation("post-tool", &payload)
            .expect("project non-mutation")
            .is_none()
    );
}
