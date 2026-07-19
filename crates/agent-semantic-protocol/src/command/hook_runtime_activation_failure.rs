use super::hook_runtime_agent_session::{
    classify_activation_failure_main_session_asp, load_asp_session_policy,
};
use super::hook_runtime_decision_render::{emit_decision, emit_hook_runtime_failure};
use super::{flag_value, string_field};
use agent_semantic_hook::{
    DecisionKind, DecisionSubject, HOOK_DECISION_SCHEMA_ID, HOOK_DECISION_SCHEMA_VERSION,
    HOOK_PROTOCOL_ID, HOOK_PROTOCOL_VERSION, HookDecision, ReasonKind, default_client_config_path,
    parse_payload,
};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

pub(super) fn emit_activation_load_failure(
    client: &str,
    event: &str,
    emit: &str,
    activation_path: &Path,
    error: &str,
    stdin: &str,
) -> Result<(), String> {
    eprintln!(
        "[agent-semantic-hook] activation disabled for this hook event: {}: {error}",
        activation_path.display()
    );
    if let Some(decision) =
        activation_failure_source_decision(client, event, activation_path, error, stdin)
    {
        return emit_decision(emit, &decision);
    }
    emit_hook_runtime_failure(
        client,
        event,
        emit,
        &format!(
            "Semantic hook activation could not be loaded; allowing tool use so activation can be repaired: {error}"
        ),
    )
}

pub(super) struct ActivationFailureMainSessionAspDecision {
    pub(super) project_root: PathBuf,
    pub(super) payload: serde_json::Value,
    pub(super) decision: HookDecision,
}

pub(super) fn activation_failure_main_session_asp_decision(
    args: &[String],
    client: &str,
    event: &str,
    stdin: &str,
) -> Option<ActivationFailureMainSessionAspDecision> {
    let payload = parse_payload(stdin).ok()?;
    let project_root = std::env::current_dir().ok()?;
    let config_path = flag_value(args, "--config")
        .map(PathBuf::from)
        .unwrap_or_else(|| default_client_config_path(&project_root.to_string_lossy()));
    let asp_session_policy = load_asp_session_policy(&config_path).ok()?;
    let decision = classify_activation_failure_main_session_asp(
        &project_root,
        client,
        event,
        &payload,
        &asp_session_policy,
    )
    .ok()
    .flatten()?;
    Some(ActivationFailureMainSessionAspDecision {
        project_root,
        payload,
        decision,
    })
}

fn activation_failure_source_decision(
    client: &str,
    event: &str,
    activation_path: &Path,
    error: &str,
    stdin: &str,
) -> Option<HookDecision> {
    let payload: serde_json::Value = serde_json::from_str(stdin).ok()?;
    let tool_name = string_field(&payload, &["tool_name", "toolName"]).unwrap_or_default();
    let tool_input = payload
        .get("tool_input")
        .or_else(|| payload.get("toolInput"))
        .or_else(|| payload.get("input"))
        .unwrap_or(&payload);
    let mut paths = Vec::new();
    super::super::hook_runtime_source_access::collect_activation_path_values(
        tool_input, &mut paths,
    );
    let command = string_field(tool_input, &["cmd", "command", "script"]);
    if command
        .as_deref()
        .is_some_and(is_activation_recovery_command)
    {
        return None;
    }
    if let Some(command) = command.as_deref() {
        super::super::hook_runtime_source_access::collect_activation_command_paths(
            command, &mut paths,
        );
    }
    paths.sort();
    paths.dedup();
    if paths.is_empty() {
        return None;
    }

    let reason_kind =
        if tool_name.eq_ignore_ascii_case("read") || tool_name.eq_ignore_ascii_case("view") {
            ReasonKind::DirectSourceRead
        } else {
            match command
                .as_deref()
                .map(agent_semantic_hook::classify_source_command_intent)
            {
                Some(agent_semantic_hook::SourceCommandIntent::DirectRead) => {
                    ReasonKind::DirectSourceRead
                }
                Some(agent_semantic_hook::SourceCommandIntent::ContentDump) => {
                    ReasonKind::BulkSourceDump
                }
                Some(agent_semantic_hook::SourceCommandIntent::Other)
                | Some(agent_semantic_hook::SourceCommandIntent::VcsDiffReview)
                | None => return None,
            }
        };
    Some(HookDecision {
        schema_id: HOOK_DECISION_SCHEMA_ID,
        schema_version: HOOK_DECISION_SCHEMA_VERSION,
        protocol_id: HOOK_PROTOCOL_ID,
        protocol_version: HOOK_PROTOCOL_VERSION,
        platform: client.to_string(),
        event: event.to_string(),
        decision: DecisionKind::Deny,
        reason_kind,
        language_ids: Vec::new(),
        subject: DecisionSubject {
            tool_name: if tool_name.is_empty() {
                None
            } else {
                Some(tool_name)
            },
            command,
            paths,
        },
        routes: Vec::new(),
        message: format!(
            "Semantic hook activation could not be loaded from {}; source reads fail closed until activation is repaired: {error}",
            activation_path.display()
        ),
        fields: BTreeMap::new(),
    })
}

fn is_activation_recovery_command(command: &str) -> bool {
    let tokens = agent_semantic_hook::semantic_shell_tokens(command);
    agent_semantic_hook::asp_invocation_indices(&tokens)
        .into_iter()
        .any(
            |asp_index| match tokens.get(asp_index + 1).map(String::as_str) {
                Some("sync") => true,
                Some("hook") => tokens
                    .get(asp_index + 2)
                    .is_some_and(|token| token == "doctor"),
                _ => false,
            },
        )
}

#[cfg(test)]
#[path = "../../tests/unit/hook_runtime_activation_recovery.rs"]
mod hook_runtime_activation_recovery_tests;
