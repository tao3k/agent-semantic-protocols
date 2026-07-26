//! Resident-child read-only permission context for hook classification.

use std::path::Path;

use agent_semantic_hook::HookDecision;

use super::hook_runtime_agent_session;

pub(super) fn classify_read_only_resident_write(
    project_root: &Path,
    client: &str,
    event: &str,
    asp_session_policy: &hook_runtime_agent_session::AspSessionPolicy,
    payload: &serde_json::Value,
) -> Option<HookDecision> {
    let sandbox_mode = super::resident_asp_explore_sandbox_mode();
    let context = resident_permission_context(
        project_root,
        asp_session_policy,
        payload,
        sandbox_mode.as_deref(),
    )?;
    agent_semantic_hook::classify_read_only_subagent_write(client, event, payload, &context)
}

pub(super) fn classify_read_only_resident_receipt(
    project_root: &Path,
    client: &str,
    event: &str,
    asp_session_policy: &hook_runtime_agent_session::AspSessionPolicy,
    payload: &serde_json::Value,
) -> Option<HookDecision> {
    let sandbox_mode = super::resident_asp_explore_sandbox_mode();
    let context = resident_permission_context(
        project_root,
        asp_session_policy,
        payload,
        sandbox_mode.as_deref(),
    )?;
    agent_semantic_hook::classify_read_only_subagent_receipt(client, event, payload, &context)
}

fn resident_permission_context<'a>(
    project_root: &Path,
    asp_session_policy: &'a hook_runtime_agent_session::AspSessionPolicy,
    payload: &'a serde_json::Value,
    sandbox_mode: Option<&'a str>,
) -> Option<agent_semantic_hook::HookSubagentPermissionContext<'a>> {
    let session_id = ["session_id", "sessionId"]
        .iter()
        .find_map(|key| payload.get(*key).and_then(serde_json::Value::as_str))?;
    let codex_hook_agent_id = ["agent_id", "agentId"]
        .iter()
        .find_map(|key| payload.get(*key).and_then(serde_json::Value::as_str));
    let codex_hook_agent_type = ["agent_type", "agentType"]
        .iter()
        .find_map(|key| payload.get(*key).and_then(serde_json::Value::as_str));
    let identity_proof = if codex_hook_agent_id.is_some()
        && codex_hook_agent_type == Some(asp_session_policy.resident_codex_agent_name())
    {
        hook_runtime_agent_session::current_session_resident_child_identity_proof(
            project_root,
            asp_session_policy,
            payload,
        )
        .ok()
        .flatten()
    } else {
        None
    };
    let live_target_proof = matches!(
        identity_proof,
        Some(crate::command::ResidentChildIdentityProof::CodexHookPayloadLiveTarget)
    );

    let managed_child_name =
        agent_semantic_hook::ManagedChildName::new(asp_session_policy.resident_child_name())?;
    let configured_codex_agent_name = agent_semantic_hook::ConfiguredCodexAgentName::new(
        asp_session_policy.resident_codex_agent_name(),
    )?;
    let configured_role =
        agent_semantic_hook::ConfiguredResidentRole::new(asp_session_policy.resident_agent_role())?;
    let session_id = agent_semantic_hook::ResidentRootSessionId::new(session_id)?;
    Some(agent_semantic_hook::HookSubagentPermissionContext::new(
        agent_semantic_hook::ResidentConfiguration {
            resident_enabled: agent_semantic_hook::ResidentEnabled::new(
                asp_session_policy.enabled(),
            ),
            managed_child_name,
            configured_codex_agent_name,
            configured_role,
        },
        agent_semantic_hook::ResidentLiveIdentity {
            codex_hook_agent_id: codex_hook_agent_id
                .and_then(agent_semantic_hook::CodexHookAgentId::new),
            codex_hook_agent_type: codex_hook_agent_type
                .and_then(agent_semantic_hook::CodexHookAgentType::new),
            resident_child_identity_proof: live_target_proof.then_some(
                agent_semantic_hook::ResidentChildIdentityProof::CodexHookPayloadLiveTarget,
            ),
            resident_child_session_id: live_target_proof.then_some(
                agent_semantic_hook::ResidentChildSessionId::new(session_id.as_str())?,
            ),
            identity_status: if live_target_proof {
                agent_semantic_hook::ResidentIdentityStatus::LiveTargetVerified
            } else {
                agent_semantic_hook::ResidentIdentityStatus::Unverified
            },
            sandbox_mode: sandbox_mode.and_then(agent_semantic_hook::ResidentSandboxMode::new),
        },
        session_id,
    ))
}
