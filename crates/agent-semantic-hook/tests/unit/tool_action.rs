use agent_semantic_hook::codex_tool_event_requires_policy_evaluation;
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
fn typed_read_action_with_path_remains_policy_bearing() {
    let payload = json!({
        "tool_name": "Read",
        "tool_input": { "file_path": "src/lib.rs" }
    });

    assert_eq!(
        codex_tool_event_requires_policy_evaluation(&payload),
        Some(true)
    );
}

#[test]
fn typed_read_actions_for_registered_document_extensions_remain_policy_bearing() {
    for path in ["ASP_ORG_SKILL.org", "RTK.md"] {
        let payload = json!({
            "tool_name": "Read",
            "tool_input": { "file_path": path }
        });

        assert_eq!(
            codex_tool_event_requires_policy_evaluation(&payload),
            Some(true),
            "registered document Read must reach the internal language-extension matcher: {path}"
        );
    }
}
