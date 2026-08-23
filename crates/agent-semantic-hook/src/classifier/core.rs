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
    let current_agent_role = payload
        .get("agent_role")
        .or_else(|| payload.get("agentRole"))
        .and_then(serde_json::Value::as_str)
        .filter(|value| !value.trim().is_empty());
    let target_role = dispatch_target_field(&decision, "targetAgentRole");
    let current_agent_roles = payload
        .get("agent_roles")
        .or_else(|| payload.get("agentRoles"))
        .and_then(serde_json::Value::as_array);
    let role_matches = target_role.is_some_and(|target| {
        current_agent_role.is_some_and(|role| role.eq_ignore_ascii_case(target))
            || current_agent_roles.is_some_and(|roles| {
                roles.iter().any(|role| {
                    role.as_str()
                        .is_some_and(|role| role.eq_ignore_ascii_case(target))
                })
            })
    });
    let registration_verified = payload
        .get("registration_verified")
        .or_else(|| payload.get("registrationVerified"))
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false);
    if registration_verified
        && current_agent.is_some()
        && current_agent_id.is_some()
        && role_matches
    {
        decision.decision = DecisionKind::Allow;
        decision.reason_kind = ReasonKind::None;
        decision.routes.clear();
        decision.message = format!(
            "Allowed: this command is already executing inside the configured typed `{}` Agent; dispatch is idempotent and must not recurse.",
            current_agent.unwrap_or("subagent")
        );
        decision.fields.insert(
            "dispatchSatisfied".to_owned(),
            serde_json::Value::Bool(true),
        );
        decision.fields.insert(
            "currentAgentType".to_owned(),
            serde_json::Value::String(current_agent.unwrap_or_default().to_owned()),
        );
        return decision;
    }

    let target_role = target_role.unwrap_or("configured");
    let receipt_kind = dispatch_target_field(&decision, "receiptKind").unwrap_or("unspecified");
    let dispatch_instruction = render_choice_plane_instruction(AgentDispatchMessageFields {
        role: target_role,
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
) -> HookDecision {
    let is_subagent = payload
        .get("is_subagent")
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false);
    if !is_subagent || decision.decision != DecisionKind::Allow {
        return decision;
    }

    let dispatch_satisfied = decision
        .fields
        .get("dispatchSatisfied")
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false);
    let registration_verified = payload
        .get("registration_verified")
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false);
    if dispatch_satisfied && registration_verified {
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

/// Classify one hook payload using a named `HookClassificationRequest`.
#[cfg(test)]
#[path = "../../tests/unit/classifier_capability.rs"]
mod capability_tests;

pub fn classify_hook_with_config(request: HookClassificationRequest<'_>) -> HookDecision {
    let actions = collect_payload_tool_actions(request.payload);
    let decision = if let Some(decision) =
        super::classify_user_prompt(request.platform, request.event, request.payload)
    {
        decision
    } else if request.event == "pre-tool"
        && let Some(candidate) = classify_tool_actions(&request, &actions)
    {
        candidate.decision
    } else {
        let subject = actions.first().map(subject_for_action).unwrap_or_default();
        allow(request.platform, request.event, subject)
    };
    let decision = enforce_registered_subagent_capability(
        resolve_dispatch_decision(decision, request.payload),
        request.payload,
    );
    let decision = super::with_selector_only_subagent_message(decision);
    let decision = with_prompt_scope_fields(decision, request.payload);
    let decision =
        with_agent_org_artifact_recovery(decision, request.config, &request.registry.project_root);
    let decision = with_hook_match_receipt(decision, request.payload, &actions, request.config);
    decision
}

fn with_hook_match_receipt(
    decision: HookDecision,
    payload: &Value,
    actions: &[ToolAction],
    config: &ClientHookConfig,
) -> HookDecision {
    let mut decision = with_action_receipt_fields(decision, payload, actions);
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
    if let Some((generation_digest, kernel_version)) = config.hook_policy_receipt() {
        decision.fields.insert(
            "hookPolicySnapshotDigest".to_string(),
            Value::String(generation_digest.to_owned()),
        );
        decision.fields.insert(
            "hookPolicyKernelVersion".to_string(),
            Value::String(kernel_version.to_owned()),
        );
        decision.fields.insert(
            "hookPolicySynchronousDependencies".to_string(),
            Value::Array(Vec::new()),
        );
    }
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
    collect_tool_actions(&tool_name, tool_input)
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
    let mut relevant = actions
        .iter()
        .filter(|action| action.operation == crate::tool_action::OperationIntent::DirectRead);
    let action = relevant.next()?;
    if relevant.next().is_some() {
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
