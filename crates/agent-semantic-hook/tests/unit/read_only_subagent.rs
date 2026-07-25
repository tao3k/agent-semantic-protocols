use agent_semantic_hook::{
    CodexHookAgentId, CodexHookAgentType, ConfiguredCodexAgentName, ConfiguredResidentRole,
    HookSubagentPermissionContext, ManagedChildName, ResidentChildIdentityProof,
    ResidentChildSessionId, ResidentConfiguration, ResidentEnabled, ResidentIdentityStatus,
    ResidentLiveIdentity, ResidentRootSessionId, ResidentSandboxMode,
    classify_read_only_subagent_write,
};
use serde_json::json;

fn resident_context(
    configured_name: &'static str,
    configured_role: &'static str,
) -> HookSubagentPermissionContext<'static> {
    resident_context_with(
        configured_name,
        configured_role,
        true,
        configured_role,
        Some(ResidentChildIdentityProof::CodexHookPayloadLiveTarget),
        "019f-root-session",
    )
}

fn resident_context_with(
    configured_name: &'static str,
    configured_role: &'static str,
    resident_enabled: bool,
    agent_type: &'static str,
    identity_proof: Option<ResidentChildIdentityProof>,
    child_session_id: &'static str,
) -> HookSubagentPermissionContext<'static> {
    HookSubagentPermissionContext::new(
        ResidentConfiguration {
            resident_enabled: ResidentEnabled::new(resident_enabled),
            managed_child_name: ManagedChildName::new(configured_name).expect("managed child name"),
            configured_codex_agent_name: ConfiguredCodexAgentName::new(configured_name)
                .expect("configured Codex agent name"),
            configured_role: ConfiguredResidentRole::new(configured_role)
                .expect("configured resident role"),
        },
        ResidentLiveIdentity {
            codex_hook_agent_id: Some(
                CodexHookAgentId::new("019f-live-agent").expect("Codex hook agent id"),
            ),
            codex_hook_agent_type: Some(
                CodexHookAgentType::new(agent_type).expect("Codex hook agent type"),
            ),
            resident_child_identity_proof: identity_proof,
            resident_child_session_id: Some(
                ResidentChildSessionId::new(child_session_id).expect("resident child session id"),
            ),
            identity_status: ResidentIdentityStatus::LiveTargetVerified,
            sandbox_mode: Some(
                ResidentSandboxMode::new("read-only").expect("resident sandbox mode"),
            ),
        },
        ResidentRootSessionId::new("019f-root-session").expect("resident root session id"),
    )
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
    let context = resident_context_with(
        "asp_explorer",
        "asp_explorer",
        true,
        "worker",
        Some(ResidentChildIdentityProof::CodexHookPayloadLiveTarget),
        "019f-root-session",
    );
    assert!(!context.resident_authorized());
}

#[test]
fn resident_authorization_rejects_missing_proof() {
    let context = resident_context_with(
        "asp_explorer",
        "asp_explorer",
        true,
        "explorer",
        None,
        "019f-root-session",
    );
    assert!(!context.resident_authorized());
}

#[test]
fn resident_authorization_rejects_wrong_session() {
    let context = resident_context_with(
        "asp_explorer",
        "asp_explorer",
        true,
        "explorer",
        Some(ResidentChildIdentityProof::CodexHookPayloadLiveTarget),
        "019f-other-session",
    );
    assert!(!context.resident_authorized());
}

#[test]
fn resident_authorization_rejects_disabled_resident() {
    let context = resident_context_with(
        "asp_explorer",
        "asp_explorer",
        false,
        "explorer",
        Some(ResidentChildIdentityProof::CodexHookPayloadLiveTarget),
        "019f-root-session",
    );
    assert!(!context.resident_authorized());
}
