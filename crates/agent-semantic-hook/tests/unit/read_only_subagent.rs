use agent_semantic_hook::{HookSubagentPermissionContext, classify_read_only_subagent_write};
use serde_json::json;

fn resident_context(
    configured_name: &'static str,
    configured_role: &'static str,
) -> HookSubagentPermissionContext<'static> {
    HookSubagentPermissionContext {
        resident_enabled: true,
        managed_child_name: configured_name,
        configured_codex_agent_name: configured_name,
        configured_role,
        codex_hook_agent_id: Some("019f-live-agent"),
        codex_hook_agent_type: Some(if configured_role == "asp_explorer" {
            "explorer"
        } else {
            configured_role
        }),
        resident_child_identity_proof: Some("codex-hook-payload-live-target"),
        resident_child_session_id: Some("019f-root-session"),
        identity_status: "live-target-verified",
        sandbox_mode: Some("read-only"),
        session_id: "019f-root-session",
    }
}

#[test]
fn asp_explorer_live_identity_does_not_recursively_deny_parser_owned_query() {
    let context = resident_context("asp_explorer", "asp_explorer");
    let payload = json!({
        "tool_name": "Bash",
        "tool_input": {"cmd": "asp rust query owner.rs items --query owner"}
    });

    assert!(context.resident_authorized());
    assert!(classify_read_only_subagent_write("codex", "pre-tool", &payload, &context).is_none());
}

#[test]
fn asp_testing_live_identity_does_not_recursively_deny_test_execution() {
    let context = resident_context("asp_testing", "asp_testing");
    let payload = json!({
        "tool_name": "Bash",
        "tool_input": {"cmd": "rtk cargo test -p agent-semantic-hook"}
    });

    assert!(context.resident_authorized());
    assert!(classify_read_only_subagent_write("codex", "pre-tool", &payload, &context).is_none());
}

#[test]
fn resident_authorization_rejects_wrong_live_type() {
    let mut context = resident_context("asp_explorer", "asp_explorer");
    context.codex_hook_agent_type = Some("worker");
    assert!(!context.resident_authorized());
}

#[test]
fn resident_authorization_rejects_wrong_proof() {
    let mut context = resident_context("asp_explorer", "asp_explorer");
    context.resident_child_identity_proof = Some("registry-exact");
    assert!(!context.resident_authorized());
}

#[test]
fn resident_authorization_rejects_wrong_session() {
    let mut context = resident_context("asp_explorer", "asp_explorer");
    context.resident_child_session_id = Some("019f-other-session");
    assert!(!context.resident_authorized());
}

#[test]
fn resident_authorization_rejects_disabled_resident() {
    let mut context = resident_context("asp_explorer", "asp_explorer");
    context.resident_enabled = false;
    assert!(!context.resident_authorized());
}
