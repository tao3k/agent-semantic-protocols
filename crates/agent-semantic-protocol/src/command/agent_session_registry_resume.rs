use std::path::Path;

use agent_semantic_client_db::{
    AgentSessionLookupRequest, AgentSessionRegistry, agent_session_unix_timestamp,
};

use super::agent_session_registry_args::SessionArgs;
use super::agent_session_registry_state::{project_session_scope_id, resolved_root_session_id};

pub(super) fn resume_session(
    registry: &AgentSessionRegistry,
    args: &SessionArgs,
    project_root: &Path,
) -> Result<(), String> {
    let project_id = project_session_scope_id(registry, project_root)?;
    registry.refresh_expired_sessions()?;
    let root_session_id = resolved_root_session_id(registry, args.root_session_id.as_deref())?;
    let name = args.name.clone();
    let mut record = registry.lookup_session(AgentSessionLookupRequest {
        project_id: &project_id,
        session_id: args.child_session_id.as_deref(),
        root_session_id: root_session_id.as_deref(),
        name: name.as_deref(),
    })?;
    let now = agent_session_unix_timestamp()?;
    let initial_registry_routable = record
        .as_ref()
        .map(|session| {
            !matches!(session.status.as_str(), "archived" | "closed") && session.is_routable_at(now)
        })
        .unwrap_or(false);
    let lookup_name = name
        .as_deref()
        .or_else(|| record.as_ref().map(|session| session.name.as_str()));
    let mut rollout_history_status = "not-needed";
    let mut rollout_history_action = "none";
    if !initial_registry_routable {
        let preflight =
            super::agent_session_registry_profile::adopt_reusable_rollout_session_before_create(
                registry,
                &project_id,
                root_session_id.as_deref(),
                args,
                lookup_name,
                record.as_ref().map(|session| session.session_id.as_str()),
                now,
            )?;
        rollout_history_status = preflight.status;
        rollout_history_action = preflight.action;
        if let Some(adopted) = preflight.record {
            record = Some(adopted);
        }
    }
    let registry_status = record
        .as_ref()
        .map(|session| session.status.as_str())
        .unwrap_or("missing");
    let registry_routable = record
        .as_ref()
        .map(|session| {
            !matches!(session.status.as_str(), "archived" | "closed") && session.is_routable_at(now)
        })
        .unwrap_or(false);
    let message_target_id = record
        .as_ref()
        .and_then(|session| session.message_target_id())
        .filter(|target_id| !target_id.trim().is_empty())
        .unwrap_or("");
    let message_target_ready = registry_routable && !message_target_id.is_empty();
    let routable = message_target_ready;
    let next_action = if registry_routable && !message_target_ready {
        "register-existing-child-with-native-message-target-or-create-managed-child"
    } else if registry_routable {
        if rollout_history_status == "adopted-reusable-rollout" {
            rollout_history_action
        } else {
            "send-follow-up-to-registered-message-target"
        }
    } else if record.is_some() {
        "create-resident-child-after-rollout-history-miss"
    } else {
        rollout_history_action
    };
    let session_id = record
        .as_ref()
        .map(|session| session.session_id.as_str())
        .unwrap_or("");
    let root_session = record
        .as_ref()
        .map(|session| session.root_session_id.as_str())
        .or(root_session_id.as_deref())
        .unwrap_or("");
    let role = record
        .as_ref()
        .map(|session| session.role.as_str())
        .unwrap_or("");
    let model = record
        .as_ref()
        .and_then(|session| session.model.as_deref())
        .unwrap_or("");
    let resolved_name = name
        .as_deref()
        .or_else(|| record.as_ref().map(|session| session.name.as_str()))
        .unwrap_or("");
    let validation = record
        .as_ref()
        .map(|session| {
            super::agent_session_registry_validation::validate_session_profile(
                &session.session_id,
                &session.root_session_id,
                &session.name,
                &session.role,
                now,
            )
        })
        .transpose()?;
    let configured_required_model =
        super::agent_session_registry_validation::expected_model_for_session_profile(
            resolved_name,
            role,
        )?;
    let required_model = validation
        .as_ref()
        .and_then(|validation| validation.expected_model.as_deref())
        .or(configured_required_model.as_deref())
        .unwrap_or("");
    let actual_model = validation
        .as_ref()
        .and_then(|validation| validation.actual_model.as_deref())
        .unwrap_or(model);
    let model_alignment_action =
        if registry_routable && !message_target_ready && !required_model.is_empty() {
            "parent-register-native-message-target-before-model-follow-up"
        } else if registry_routable && !required_model.is_empty() {
            "parent-send-codex-follow-up-with-required-model-override-and-revalidate"
        } else if !required_model.is_empty() {
            "parent-create-resident-child-with-required-model-and-revalidate"
        } else {
            "none"
        };
    let model_alignment_message = if registry_routable
        && !message_target_ready
        && !required_model.is_empty()
    {
        format!(
            "The resident child is lifecycle-valid but has no native message-agent target. The parent must register the existing child with its native Codex message target or create a managed ASP child with model override {required_model} and light/low reasoning, then rerun asp agent session status --name {resolved_name}."
        )
    } else if registry_routable && !required_model.is_empty() {
        format!(
            "After native resume, the parent must send a Codex follow-up to this same child thread with model override {required_model} and light/low reasoning, wait for the child receipt, then rerun asp agent session status --name {resolved_name}."
        )
    } else if !required_model.is_empty() {
        format!(
            "No routable resident child is available. The parent must create the resident ASP child with model override {required_model} and light/low reasoning, register it as {resolved_name}, wait for the child receipt, then rerun asp agent session status --name {resolved_name}."
        )
    } else {
        "none".to_string()
    };
    println!(
        "[agent-session-resume] owner=rust name=\"{}\" session=\"{}\" rootSession=\"{}\" registryStatus=\"{}\" registryRoutable={} routable={} role=\"{}\" model=\"{}\" requiredModel=\"{}\" actualModel=\"{}\" modelAlignmentAction=\"{}\" modelAlignmentMessage=\"{}\" rolloutHistoryStatus=\"{}\" rolloutHistoryAction=\"{}\" messageTargetStatus=\"{}\" messageTargetResultSource=\"{}\" messageAgentTargetId=\"{}\" nextAction=\"{}\" db=\"{}\"",
        resume_field(resolved_name),
        resume_field(session_id),
        resume_field(root_session),
        resume_field(registry_status),
        registry_routable,
        routable,
        resume_field(role),
        resume_field(model),
        resume_field(required_model),
        resume_field(actual_model),
        resume_field(model_alignment_action),
        resume_field(&model_alignment_message),
        resume_field(rollout_history_status),
        resume_field(rollout_history_action),
        if message_target_ready {
            "ready"
        } else {
            "missing"
        },
        if message_target_ready {
            "registry-message-target-id"
        } else if record.is_some() {
            "registry-message-target-id-missing"
        } else {
            "none"
        },
        resume_field(message_target_id),
        resume_field(next_action),
        resume_field(&registry.db_path().display().to_string())
    );
    Ok(())
}

fn resume_field(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}
