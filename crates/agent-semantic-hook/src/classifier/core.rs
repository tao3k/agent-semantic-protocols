//! Hook classifier orchestration for `agent-semantic-hook`.

use std::borrow::Cow;

use serde_json::Value;

use super::agent_org_artifacts::with_agent_org_artifact_recovery;
use super::decision::allow;
use crate::agent_dispatch_message::{AgentDispatchMessageFields, render_choice_plane_instruction};
use crate::{
    ClientHookConfig, DecisionKind, HookDecision, HookRuntime, OperationIntent, ReasonKind,
    ToolAction, collect_tool_actions, payload_string, subject_for_action,
};

use super::higher_priority_candidate;

/// Named input for hook classification with optional client policy config.
pub struct HookClassificationRequest<'a> {
    /// Activated provider runtime for the current project.
    pub registry: &'a HookRuntime,
    /// Project-local client rules layered over the built-in classifier.
    pub config: &'a ClientHookConfig,
    /// Hook client identifier such as `codex`.
    pub platform: &'a str,
    /// Canonical hook event name such as `pre-tool`.
    pub event: &'a str,
    /// Raw platform hook payload.
    pub payload: &'a Value,
}

/// Classify one platform hook payload against an activated provider runtime.
pub fn classify_hook(
    registry: &HookRuntime,
    platform: &str,
    event: &str,
    payload: &Value,
) -> HookDecision {
    classify_hook_with_config(HookClassificationRequest {
        registry,
        config: &ClientHookConfig::default(),
        platform,
        event,
        payload,
    })
}

fn dispatch_target_field<'a>(decision: &'a HookDecision, field: &str) -> Option<&'a str> {
    decision
        .fields
        .get(field)
        .and_then(serde_json::Value::as_str)
        .filter(|value| !value.trim().is_empty())
}

fn registered_profile_allows_decision_intent(
    decision: &HookDecision,
    payload: &serde_json::Value,
) -> bool {
    let Some(intent) = decision
        .fields
        .get("intent")
        .and_then(serde_json::Value::as_str)
    else {
        return false;
    };
    payload
        .get("registered_allowed_rule_intents")
        .or_else(|| payload.get("registeredAllowedRuleIntents"))
        .and_then(serde_json::Value::as_array)
        .is_some_and(|allowed| {
            allowed
                .iter()
                .any(|candidate| candidate.as_str() == Some(intent))
        })
}

pub(super) fn resolve_dispatch_decision(
    mut decision: HookDecision,
    payload: &serde_json::Value,
) -> HookDecision {
    if decision.decision == DecisionKind::Allow
        || decision.reason_kind != ReasonKind::SubagentReceiptRequired
    {
        return decision;
    }
    let current_agent = payload
        .get("agent_type")
        .or_else(|| payload.get("agentType"))
        .and_then(serde_json::Value::as_str)
        .filter(|value| !value.trim().is_empty());
    let current_agent_id = payload
        .get("agent_id")
        .or_else(|| payload.get("agentId"))
        .and_then(serde_json::Value::as_str)
        .filter(|value| !value.trim().is_empty());
    let registration_verified = payload
        .get("registration_verified")
        .or_else(|| payload.get("registrationVerified"))
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false);
    let registered_agent_name = registration_verified
        .then(|| {
            payload
                .get("registered_agent_name")
                .or_else(|| payload.get("registeredAgentName"))
                .and_then(serde_json::Value::as_str)
                .filter(|value| !value.trim().is_empty())
        })
        .flatten();
    let target_agent = dispatch_target_field(&decision, "targetAgent");
    let agent_matches = target_agent.is_some_and(|target| {
        registered_agent_name.is_some_and(|registered| {
            registered
                .chars()
                .map(|ch| if ch == '-' { '_' } else { ch })
                .eq(target.chars().map(|ch| if ch == '-' { '_' } else { ch }))
        })
    });
    let registered_intent_matches = registered_profile_allows_decision_intent(&decision, payload);
    if registration_verified
        && current_agent.is_some()
        && current_agent_id.is_some()
        && registered_agent_name.is_some()
        && agent_matches
        && registered_intent_matches
    {
        decision.decision = DecisionKind::Allow;
        decision.reason_kind = ReasonKind::None;
        decision.routes.clear();
        decision.message = format!(
            "Allowed: this command is already executing inside the configured typed `{}` Agent; dispatch is idempotent and must not recurse.",
            registered_agent_name.unwrap_or("subagent")
        );
        decision.fields.insert(
            "dispatchSatisfied".to_owned(),
            serde_json::Value::Bool(true),
        );
        decision.fields.insert(
            "currentAgentType".to_owned(),
            serde_json::Value::String(current_agent.unwrap_or_default().to_owned()),
        );
        decision.fields.insert(
            "currentRegisteredAgentName".to_owned(),
            serde_json::Value::String(registered_agent_name.unwrap_or_default().to_owned()),
        );
        decision.fields.insert(
            "dispatchAdmission".to_owned(),
            serde_json::Value::String(
                if registered_intent_matches {
                    "verified-registration-intent"
                } else {
                    "verified-registration-agent"
                }
                .to_owned(),
            ),
        );
        return decision;
    }

    let target_agent = target_agent.unwrap_or("configured");
    let receipt_kind = dispatch_target_field(&decision, "receiptKind").unwrap_or("unspecified");
    let target_symbol = dispatch_target_field(&decision, "targetAgentSymbol")
        .map(str::to_owned)
        .unwrap_or_else(|| format!("@{target_agent}"));
    let dispatch_instruction = render_choice_plane_instruction(AgentDispatchMessageFields {
        agent: target_agent,
        symbol: Some(&target_symbol),
        receipt_kind,
    });
    if decision.message.trim().is_empty() {
        decision.message = dispatch_instruction;
    } else if !decision.message.contains(&dispatch_instruction) {
        decision.message = format!("{}\n{dispatch_instruction}", decision.message);
    }
    decision.fields.insert(
        "dispatchGuidance".to_owned(),
        serde_json::Value::String("delegate-exact-command-to-typed-agent".to_owned()),
    );
    decision
}

fn enforce_registered_subagent_capability(
    mut decision: HookDecision,
    payload: &serde_json::Value,
    actions: &[ToolAction],
) -> HookDecision {
    if is_explicit_no_agent_host_bypass(&decision) {
        return decision;
    }
    let is_subagent = payload
        .get("is_subagent")
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false);
    if !is_subagent {
        return decision;
    }

    let registration_verified = payload
        .get("registration_verified")
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false);
    let edit_is_denied = payload
        .get("registered_denied_actions")
        .or_else(|| payload.get("registeredDeniedActions"))
        .and_then(serde_json::Value::as_array)
        .is_some_and(|denied| denied.iter().any(|action| action.as_str() == Some("edit")));
    let attempts_edit = actions.iter().any(|action| {
        action.host_action == crate::action_ir::HostInvocationKind::Edit
            || action
                .derive_agent_action()
                .filesystem_permissions
                .iter()
                .any(|permission| {
                    permission.permission == crate::action_ir::FilesystemPermissionKind::Write
                })
    });
    if registration_verified && edit_is_denied && attempts_edit {
        let agent_name = payload
            .get("registered_agent_name")
            .or_else(|| payload.get("registeredAgentName"))
            .and_then(serde_json::Value::as_str)
            .filter(|name| !name.trim().is_empty())
            .unwrap_or("read-only subagent");
        decision.decision = DecisionKind::Deny;
        decision.reason_kind = ReasonKind::ReadOnlySubagentWrite;
        decision.message =
            format!("编辑模式必须在主线程完成，当前的 {agent_name} 不允许执行 edit action。");
        decision.fields.insert(
            "registeredAgentName".to_owned(),
            serde_json::Value::String(agent_name.to_owned()),
        );
        decision.fields.insert(
            "permissionAction".to_owned(),
            serde_json::Value::String("edit".to_owned()),
        );
        decision.fields.insert(
            "requiredAction".to_owned(),
            serde_json::Value::String("complete-edit-in-main-thread".to_owned()),
        );
        return decision;
    }
    let registered_agent_name = payload
        .get("registered_agent_name")
        .or_else(|| payload.get("registeredAgentName"))
        .and_then(serde_json::Value::as_str)
        .filter(|name| !name.trim().is_empty())
        .unwrap_or("registered subagent");
    let allowed_rule_intents = payload
        .get("registered_allowed_rule_intents")
        .or_else(|| payload.get("registeredAllowedRuleIntents"))
        .and_then(serde_json::Value::as_array);
    let matched_rule_intent = decision
        .fields
        .get("intent")
        .and_then(serde_json::Value::as_str)
        .map(str::to_owned);
    let profile_allows_intent = registered_profile_allows_decision_intent(&decision, payload);
    if registration_verified && !profile_allows_intent {
        let attempted = matched_rule_intent.as_deref().unwrap_or("unscoped-command");
        let allowed = allowed_rule_intents
            .into_iter()
            .flatten()
            .filter_map(serde_json::Value::as_str)
            .collect::<Vec<_>>()
            .join(", ");
        decision.decision = DecisionKind::Deny;
        decision.reason_kind = ReasonKind::SubagentCapabilityDenied;
        decision.message = format!(
            "当前的 {registered_agent_name} Profile 不允许执行 Rule intent `{attempted}`；仅允许：{allowed}。"
        );
        decision.fields.insert(
            "registeredAgentName".to_owned(),
            serde_json::Value::String(registered_agent_name.to_owned()),
        );
        decision.fields.insert(
            "attemptedRuleIntent".to_owned(),
            serde_json::Value::String(attempted.to_owned()),
        );
        decision.fields.insert(
            "capabilityAdmission".to_owned(),
            serde_json::Value::String("agent-profile-rule-intent-scope".to_owned()),
        );
        return decision;
    }
    if decision.decision != DecisionKind::Allow {
        return decision;
    }
    if registration_verified && profile_allows_intent {
        decision.fields.insert(
            "capabilityAdmission".to_owned(),
            serde_json::Value::String("verified-registration-intent".to_owned()),
        );
        decision.fields.insert(
            "registeredAgentName".to_owned(),
            serde_json::Value::String(registered_agent_name.to_owned()),
        );
        return decision;
    }

    decision.decision = DecisionKind::Deny;
    decision.reason_kind = ReasonKind::SubagentReceiptRequired;
    decision.message =
        "registered subagent capability receipt is required before tool execution".to_owned();
    decision.fields.insert(
        "requiredAction".to_owned(),
        serde_json::Value::String("dispatch-registered-agent-capability".to_owned()),
    );
    decision.fields.insert(
        "capabilityAdmission".to_owned(),
        serde_json::Value::String("registered-route-required".to_owned()),
    );
    decision
}

fn is_explicit_no_agent_host_bypass(decision: &HookDecision) -> bool {
    decision.decision == DecisionKind::Allow
        && decision
            .fields
            .get("bypassOwner")
            .and_then(serde_json::Value::as_str)
            == Some("hook-matcher")
        && decision
            .fields
            .get("bypassScope")
            .and_then(serde_json::Value::as_str)
            == Some("host-policy")
}

/// Classify one hook payload using a named `HookClassificationRequest`.
#[cfg(test)]
#[path = "../../tests/unit/classifier_capability.rs"]
mod capability_tests;

pub fn classify_hook_with_config(request: HookClassificationRequest<'_>) -> HookDecision {
    let actions = collect_payload_tool_actions(request.payload);
    let tool_policy = classify_tool_actions(&request, &actions);
    if tool_policy
        .as_ref()
        .is_some_and(|candidate| is_explicit_no_agent_host_bypass(&candidate.decision))
    {
        return with_hook_match_receipt(
            tool_policy
                .expect("checked explicit bypass candidate")
                .decision,
            request.payload,
            &actions,
            request.config,
            request.registry,
        );
    }
    let decision = if let Some(decision) =
        super::classify_user_prompt(request.platform, request.event, request.payload)
    {
        decision
    } else if request.event == "pre-tool"
        && let Some(candidate) = tool_policy
    {
        candidate.decision
    } else {
        let subject = actions.first().map(subject_for_action).unwrap_or_default();
        allow(request.platform, request.event, subject)
    };
    if is_explicit_no_agent_host_bypass(&decision) {
        return with_hook_match_receipt(
            decision,
            request.payload,
            &actions,
            request.config,
            request.registry,
        );
    }
    let decision = resolve_dispatch_decision(decision, request.payload);
    let mut decision = if matches!(request.event, "pre-tool" | "permission-request") {
        enforce_registered_subagent_capability(decision, request.payload, &actions)
    } else {
        decision
    };
    if decision.reason_kind == ReasonKind::RegisteredSourceRouteRequired {
        super::materialize_source_access_deny_message(&mut decision);
    }
    let decision = super::with_selector_only_subagent_message(decision);
    let decision = with_prompt_scope_fields(decision, request.payload);
    let decision =
        with_agent_org_artifact_recovery(decision, request.config, &request.registry.project_root);
    let decision = with_hook_match_receipt(
        decision,
        request.payload,
        &actions,
        request.config,
        request.registry,
    );
    decision
}

fn with_hook_match_receipt(
    decision: HookDecision,
    payload: &Value,
    actions: &[ToolAction],
    config: &ClientHookConfig,
    registry: &HookRuntime,
) -> HookDecision {
    let had_agent_action = decision.fields.contains_key("agentAction");
    let mut decision = with_action_receipt_fields(decision, payload, actions);
    if let Some(action) = actions.first()
        && !had_agent_action
        && decision.language_ids.is_empty()
        && !decision.fields.contains_key("configRuleId")
        && !action.paths.is_empty()
    {
        let (agent_action_receipt, language_ids) = config.observed_action_receipt(registry, action);
        decision
            .fields
            .insert("agentAction".to_owned(), agent_action_receipt);
        decision.language_ids = language_ids;
    }
    if let Some(command) = payload_command(payload) {
        // Compound commands are matched per parser-owned stage, but a Hook
        // receipt must retain the exact Host envelope that was authorized or
        // denied rather than a re-rendered inner stage.
        decision.subject.command = Some(command);
    }
    if decision.reason_kind == ReasonKind::SubagentReceiptRequired
        && let Some(command) = payload_command(payload)
        && !decision.message.contains(&command)
    {
        decision
            .message
            .push_str(&format!("\nDenied command: `{command}`."));
    }
    config.attach_hook_policy_receipt(&mut decision);
    decision
}

fn payload_command(payload: &Value) -> Option<String> {
    ["tool_input", "toolInput", "parameters", "input"]
        .into_iter()
        .filter_map(|key| payload.get(key))
        .find_map(|input| {
            ["cmd", "command"]
                .into_iter()
                .find_map(|key| input.get(key).and_then(Value::as_str))
        })
        .map(str::to_owned)
}

pub(super) fn with_action_receipt_fields(
    mut decision: HookDecision,
    payload: &Value,
    actions: &[ToolAction],
) -> HookDecision {
    if !decision.fields.contains_key("agentAction")
        && let Some(action) = actions.first()
    {
        decision.fields.insert(
            "agentAction".to_string(),
            action.derive_agent_action().receipt_value(),
        );
    }
    let payload_keys = payload
        .as_object()
        .map(|object| object.keys().cloned().collect::<Vec<_>>())
        .unwrap_or_default();
    decision.fields.insert(
        "hookPayloadKeys".to_string(),
        Value::Array(payload_keys.into_iter().map(Value::String).collect()),
    );

    let normalized_actions = actions
        .iter()
        .filter(|action| {
            !action.tool_name.is_empty()
                || action.command.is_some()
                || !action.paths.is_empty()
                || action.operation != OperationIntent::Unknown
        })
        .map(|action| {
            serde_json::json!({
                "toolName": action.tool_name,
                "toolSurface": action.surface.as_str(),
                "operationIntent": action.operation.as_str(),
                "paths": action.paths,
            })
        })
        .collect();
    decision.fields.insert(
        "normalizedActions".to_string(),
        Value::Array(normalized_actions),
    );
    decision
}

fn with_prompt_scope_fields(mut decision: HookDecision, payload: &Value) -> HookDecision {
    if let Some(session_id) =
        payload_string(payload, "session_id").or_else(|| payload_string(payload, "sessionId"))
    {
        decision
            .fields
            .entry("sessionId".to_string())
            .or_insert_with(|| Value::String(session_id));
    }
    if let Some(transcript_path) = payload_string(payload, "transcript_path")
        .or_else(|| payload_string(payload, "transcriptPath"))
    {
        decision
            .fields
            .entry("transcriptPath".to_string())
            .or_insert_with(|| Value::String(transcript_path));
    }
    decision
}

pub(super) fn collect_payload_tool_actions(payload: &Value) -> Vec<ToolAction> {
    let tool_name = payload_string(payload, "tool_name")
        .or_else(|| payload_string(payload, "toolName"))
        .unwrap_or_default();
    let tool_input = payload
        .get("tool_input")
        .or_else(|| payload.get("toolInput"))
        .or_else(|| payload.get("parameters"))
        .or_else(|| payload.get("input"))
        .or_else(|| payload.get("arguments"))
        .unwrap_or(payload);
    let host_action = crate::tool_action_host_binding::plugin_host_action(payload)
        .unwrap_or(crate::action_ir::HostInvocationKind::Unknown);
    let mut actions = collect_tool_actions(&tool_name, tool_input);
    for action in &mut actions {
        action.host_action = host_action;
    }
    actions
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DirectReadSourceKey {
    pub path: String,
    pub extension: String,
    pub tool_name: String,
}

/// Normalized lookup key for a wrapped registered-source decision shard.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ShellReadSourceKey {
    pub path: String,
    pub extension: String,
    pub command: String,
    pub command_tokens: Vec<String>,
    pub tool_name: String,
}

/// Return the normalized path and extension key for the direct-read matcher.
///
/// This consumes the same ToolAction normalization as the full policy engine;
/// no Host tool name is matched by the cache layer itself.
pub fn direct_read_source_key(payload: &Value) -> Option<DirectReadSourceKey> {
    let actions = collect_payload_tool_actions(payload);
    direct_read_source_key_from_actions(&actions)
}

pub(super) fn direct_read_source_key_from_actions(
    actions: &[ToolAction],
) -> Option<DirectReadSourceKey> {
    let [action] = actions else {
        return None;
    };
    if action.operation != crate::tool_action::OperationIntent::DirectRead {
        return None;
    }
    let [path] = action.paths.as_slice() else {
        return None;
    };
    let dot = path.rfind('.')?;
    Some(DirectReadSourceKey {
        path: path.clone(),
        extension: path[dot..].to_ascii_lowercase(),
        tool_name: action.tool_name.clone(),
    })
}

pub fn direct_read_source_extension(payload: &Value) -> Option<String> {
    direct_read_source_key(payload).map(|key| key.extension)
}

/// Return every normalized command/path key for a one-action shell read.
///
/// A wrapped Host envelope may contain several dotted arguments (for example
/// an activation artifact and the actual registered source). The published
/// Binary v1 section index, rather than a lexical first/last-path heuristic,
/// owns which extensions are registered and therefore admissible fast-path
/// candidates.
pub fn shell_read_source_keys(payload: &Value) -> Vec<ShellReadSourceKey> {
    let actions = collect_payload_tool_actions(payload);
    shell_read_source_keys_from_actions(&actions)
}

pub(super) fn shell_read_source_keys_from_actions(
    actions: &[ToolAction],
) -> Vec<ShellReadSourceKey> {
    let mut relevant = actions.iter().filter(|action| {
        action.surface == crate::tool_action::ToolSurface::CodexShell
            && action.operation == crate::tool_action::OperationIntent::ShellCommand
    });
    let Some(action) = relevant.next() else {
        return Vec::new();
    };
    if relevant.next().is_some() {
        return Vec::new();
    }
    let Some(command) = action.command.clone() else {
        return Vec::new();
    };
    let Some(command_tokens) = action.command_tokens().map(Cow::into_owned) else {
        return Vec::new();
    };
    let mut seen = std::collections::BTreeSet::new();
    action
        .paths
        .iter()
        .filter_map(|path| {
            let dot = path.rfind('.')?;
            let extension = path[dot..].to_ascii_lowercase();
            seen.insert((extension.clone(), path.clone()))
                .then(|| ShellReadSourceKey {
                    path: path.clone(),
                    extension,
                    command: command.clone(),
                    command_tokens: command_tokens.clone(),
                    tool_name: action.tool_name.clone(),
                })
        })
        .collect()
}

/// Return the first normalized command/path key for compatibility callers.
pub fn shell_read_source_key(payload: &Value) -> Option<ShellReadSourceKey> {
    shell_read_source_keys(payload).into_iter().next()
}

pub(crate) fn default_allow_for_normalized_action(
    platform: &str,
    event: &str,
    action: &ToolAction,
) -> HookDecision {
    allow(platform, event, subject_for_action(action))
}

/// Evaluate the config-independent provider-binary admission before any
/// config-compiled command shard. Its priority is higher than user policy, so a
/// direct provider executable cannot be reinterpreted as a testing profile.
fn classify_tool_actions(
    request: &HookClassificationRequest<'_>,
    actions: &[ToolAction],
) -> Option<crate::hook_config::HookPolicyCandidate> {
    let HookClassificationRequest {
        registry,
        config,
        platform,
        event,
        payload: _,
    } = request;
    let mut highest_denial = None;
    let mut highest_allow = None;
    for action in actions {
        let config_candidate = config.classify_candidate(registry, platform, event, action);
        let Some(candidate) = config_candidate else {
            continue;
        };
        if candidate.decision.decision == crate::DecisionKind::Allow && candidate.terminal {
            return Some(candidate);
        }
        match candidate.decision.decision {
            crate::DecisionKind::Allow => {
                highest_allow = higher_priority_candidate(highest_allow, Some(candidate));
            }
            crate::DecisionKind::Block | crate::DecisionKind::Deny => {
                highest_denial = higher_priority_candidate(highest_denial, Some(candidate));
            }
        }
    }
    highest_denial.or(highest_allow)
}
