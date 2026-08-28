use agent_semantic_hook::{
    HookRuntime, bind_plugin_host_matcher, classify_hook,
    codex_tool_event_requires_policy_evaluation,
};
use serde_json::json;

#[test]
fn unrelated_typed_action_with_path_does_not_become_a_read_action() {
    let payload = json!({
        "tool_name": "functions.view_image",
        "tool_input": { "path": "/tmp/example.png" }
    });

    assert_eq!(
        codex_tool_event_requires_policy_evaluation(&payload),
        Some(false),
        "a path-shaped field cannot override an unrelated typed action identity"
    );
}

#[test]
fn plugin_host_action_binding_is_exact_and_materialized_in_action_ir() {
    let mut payload = json!({
        "tool_name": "apply_patch",
        "tool_input": { "command": "*** Begin Patch\n*** End Patch" }
    });
    bind_plugin_host_matcher(&mut payload, Some("apply_patch"), None)
        .expect("bind canonical apply_patch matcher");
    let runtime = HookRuntime {
        project_root: ".".to_owned(),
        rankers: Vec::new(),
        providers: Vec::new(),
        policy_providers: Vec::new(),
    };
    let decision = classify_hook(&runtime, "codex", "pre-tool", &payload);
    assert_eq!(
        decision.fields["agentAction"]["hostInvocation"]["action"],
        "edit"
    );
}

#[test]
fn plugin_host_matcher_binding_rejects_mismatch_and_unknown_matcher() {
    let payload = json!({
        "tool_name": "Bash",
        "tool_input": { "command": "true" }
    });
    let mut mismatch = payload.clone();
    assert!(
        bind_plugin_host_matcher(&mut mismatch, Some("apply_patch"), None)
            .unwrap_err()
            .contains("binding mismatch")
    );
    let mut unknown = payload;
    assert!(
        bind_plugin_host_matcher(&mut unknown, Some("Delete"), None)
            .unwrap_err()
            .contains("unknown plugin Host matcher")
    );
}

#[test]
fn plugin_host_action_binding_supports_the_declared_mcp_family() {
    let mut payload = json!({
        "tool_name": "mcp__filesystem__read_file",
        "tool_input": { "path": "src/lib.rs" }
    });
    bind_plugin_host_matcher(&mut payload, None, Some("mcp__")).expect("bind MCP matcher family");
    let runtime = HookRuntime {
        project_root: ".".to_owned(),
        rankers: Vec::new(),
        providers: Vec::new(),
        policy_providers: Vec::new(),
    };
    let decision = classify_hook(&runtime, "codex", "pre-tool", &payload);
    assert_eq!(
        decision.fields["agentAction"]["hostInvocation"]["action"],
        "mcp"
    );
}
