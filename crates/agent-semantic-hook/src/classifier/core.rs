// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Hook classifier orchestration for `agent-semantic-hook`.

use std::borrow::Cow;

use serde_json::Value;

use super::agent_org_artifacts::with_agent_org_artifact_recovery;
use super::decision::allow;
use crate::ClientHookConfig;
use crate::DecisionKind;
use crate::HookDecision;
use crate::HookRuntime;
use crate::OperationIntent;
use crate::ReasonKind;
use crate::ToolAction;
use crate::agent_dispatch_message::render_collaboration_instruction;
use crate::collect_tool_actions;
use crate::payload_string;
use crate::subject_for_action;

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
        || decision.reason_kind != ReasonKind::AgentChoiceRequired
    {
        return decision;
    }
    let agent_role = payload
        .get("agent_role")
        .or_else(|| payload.get("agentRole"))
        .or_else(|| payload.get("agent_type"))
        .or_else(|| payload.get("agentType"))
        .and_then(serde_json::Value::as_str)
        .filter(|value| !value.trim().is_empty());
    let target_agent = dispatch_target_field(&decision, "targetAgent");
    let agent_matches = target_agent.is_some_and(|target| {
        agent_role.is_some_and(|role| {
            role.chars()
                .map(|ch| if ch == '-' { '_' } else { ch })
                .eq(target.chars().map(|ch| if ch == '-' { '_' } else { ch }))
        })
    });
    if agent_matches {
        decision.decision = DecisionKind::Allow;
        decision.reason_kind = ReasonKind::None;
        decision.routes.clear();
        decision.message = format!(
            "Allowed: this command is already executing inside the Config-selected `{}` Agent; dispatch is idempotent and must not recurse.",
            agent_role.unwrap_or("agent")
        );
        decision.fields.insert(
            "dispatchSatisfied".to_owned(),
            serde_json::Value::Bool(true),
        );
        decision.fields.insert(
            "currentAgentRole".to_owned(),
            serde_json::Value::String(agent_role.unwrap_or_default().to_owned()),
        );
        decision.fields.insert(
            "dispatchAdmission".to_owned(),
            serde_json::Value::String("config-agent-role".to_owned()),
        );
        return decision;
    }

    let target_agent = decision
        .fields
        .get("targetAgent")
        .and_then(serde_json::Value::as_str);
    let parent_session_id = payload
        .get("session_id")
        .or_else(|| payload.get("sessionId"))
        .and_then(serde_json::Value::as_str);
    let parent_task = decision.routes.first().map_or_else(
        || {
            let tool_name = payload
                .get("tool_name")
                .or_else(|| payload.get("toolName"))
                .and_then(serde_json::Value::as_str)
                .unwrap_or("unknown-host-tool");
            let tool_input = payload
                .get("tool_input")
                .or_else(|| payload.get("toolInput"))
                .cloned()
                .unwrap_or(serde_json::Value::Null);
            format!(
                "Invoke Host tool `{tool_name}` exactly once with input {}",
                serde_json::to_string(&tool_input).expect("encode parent Host tool input")
            )
        },
        |route| {
            format!(
                "Execute this exact argv once: {}",
                serde_json::to_string(&route.argv).expect("encode parent route argv")
            )
        },
    );
    let dispatch_instruction =
        render_collaboration_instruction(target_agent, parent_session_id, &parent_task);
    if decision.message.trim().is_empty() {
        decision.message = dispatch_instruction;
    } else if !decision.message.contains(&dispatch_instruction) {
        decision.message = format!("{}\n{dispatch_instruction}", decision.message);
    }
    decision.fields.insert(
        "dispatchGuidance".to_owned(),
        serde_json::Value::String("delegate-exact-command-to-typed-agent".to_owned()),
    );
    decision.fields.insert(
        "parentDispatchTask".to_owned(),
        serde_json::Value::String(parent_task),
    );
    decision
}

fn is_explicit_host_policy_passthrough(decision: &HookDecision) -> bool {
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
        .is_some_and(|candidate| is_explicit_host_policy_passthrough(&candidate.decision))
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
    if is_explicit_host_policy_passthrough(&decision) {
        return with_hook_match_receipt(
            decision,
            request.payload,
            &actions,
            request.config,
            request.registry,
        );
    }
    let mut decision = resolve_dispatch_decision(decision, request.payload);
    if decision.reason_kind == ReasonKind::RegisteredSourceRouteRequired {
        super::materialize_source_access_deny_message(&mut decision);
    }
    let decision = super::with_executable_evidence_subagent_message(decision);
    let decision = with_prompt_scope_fields(decision, request.payload);
    let decision =
        with_agent_org_artifact_recovery(decision, request.config, &request.registry.project_root);
    with_hook_match_receipt(
        decision,
        request.payload,
        &actions,
        request.config,
        request.registry,
    )
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
    if decision.reason_kind == ReasonKind::AgentChoiceRequired
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

/// Normalized lookup key for a wrapped registered-source decision shard.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ShellReadSourceKey {
    pub path: String,
    pub extension: String,
    pub command: String,
    pub command_tokens: Vec<String>,
    pub tool_name: String,
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
