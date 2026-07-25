//! Write-intent classification for read-only resident children.

use std::collections::BTreeMap;

use serde_json::Value;

use super::identity::HookSubagentPermissionContext;
use super::payload::string_field;
use crate::protocol::{
    DecisionKind, DecisionSubject, HOOK_DECISION_SCHEMA_ID, HOOK_DECISION_SCHEMA_VERSION,
    HOOK_PROTOCOL_ID, HOOK_PROTOCOL_VERSION, HookDecision, ReasonKind,
};
use crate::tool_action::{ToolAction, collect_tool_actions};

struct ToolWriteIntent {
    tool_name: String,
    command: Option<String>,
    paths: Vec<String>,
}

pub fn classify_read_only_subagent_write(
    platform: &str,
    event: &str,
    payload: &Value,
    context: &HookSubagentPermissionContext<'_>,
) -> Option<HookDecision> {
    if event != "pre-tool" || !context.resident_authorized() || !context.is_read_only_sandbox() {
        return None;
    }
    let intent = payload_write_intent(payload)?;

    let mut fields = BTreeMap::new();
    fields.insert(
        "sessionId".to_string(),
        Value::String(context.session_id().to_string()),
    );
    fields.insert(
        "residentChildName".to_string(),
        Value::String(context.managed_child_name().to_string()),
    );
    fields.insert(
        "readOnlyAgentName".to_string(),
        Value::String(context.configured_codex_agent_name().to_string()),
    );
    fields.insert(
        "readOnlyAgentRole".to_string(),
        Value::String(context.configured_role().to_string()),
    );
    fields.insert(
        "codexHookAgentId".to_string(),
        Value::String(
            context
                .codex_hook_agent_id()
                .unwrap_or_default()
                .to_string(),
        ),
    );
    fields.insert(
        "codexHookAgentType".to_string(),
        Value::String(
            context
                .codex_hook_agent_type()
                .unwrap_or_default()
                .to_string(),
        ),
    );
    fields.insert(
        "residentChildIdentityProof".to_string(),
        Value::String(
            context
                .resident_child_identity_proof()
                .unwrap_or_default()
                .to_string(),
        ),
    );
    fields.insert(
        "readOnlySessionId".to_string(),
        Value::String(context.session_id().to_string()),
    );
    fields.insert(
        "residentAuthorizationStatus".to_string(),
        Value::String(context.identity_status().to_string()),
    );
    fields.insert(
        "configuredSandboxMode".to_string(),
        Value::String(context.sandbox_mode().unwrap_or_default().to_string()),
    );
    fields.insert(
        "requiredAction".to_string(),
        Value::String("return-read-only-search-evidence".to_string()),
    );
    fields.insert(
        "operationIntent".to_string(),
        Value::String("write-edit".to_string()),
    );

    Some(HookDecision {
        schema_id: HOOK_DECISION_SCHEMA_ID,
        schema_version: HOOK_DECISION_SCHEMA_VERSION,
        protocol_id: HOOK_PROTOCOL_ID,
        protocol_version: HOOK_PROTOCOL_VERSION,
        platform: platform.to_string(),
        event: event.to_string(),
        decision: DecisionKind::Deny,
        reason_kind: ReasonKind::ReadOnlySubagentWrite,
        language_ids: Vec::new(),
        subject: DecisionSubject {
            tool_name: Some(intent.tool_name.clone()),
            command: intent.command,
            paths: intent.paths,
        },
        routes: Vec::new(),
        message: format!(
            "read-only ASP-managed session `{}` denied write/edit tool `{}`. \
             This ASP search child may only run ASP search/query, owner/frontier ranking, \
             dependency, and test reachability commands. Return one selector-only graph-route \
             `[asp-search-subagent]` receipt with schema/intent/route/state/evidence/next; do not return source bodies, \
             snippets, or line-range selectors. Route exact reads, edits, and validation to the \
             parent agent or a non-search worker.",
            context.session_id(),
            intent.tool_name
        ),
        fields,
    })
}

fn payload_write_intent(payload: &Value) -> Option<ToolWriteIntent> {
    collect_payload_tool_actions(payload)
        .into_iter()
        .find_map(|action| action_write_intent(&action))
}

fn collect_payload_tool_actions(payload: &Value) -> Vec<ToolAction> {
    let Some(tool_name) = string_field(payload, &["tool_name", "toolName"]) else {
        return Vec::new();
    };
    let tool_input = payload
        .get("tool_input")
        .or_else(|| payload.get("toolInput"))
        .or_else(|| payload.get("parameters"))
        .or_else(|| payload.get("input"))
        .or_else(|| payload.get("arguments"))
        .unwrap_or(payload);
    collect_tool_actions(&tool_name, tool_input)
}

fn action_write_intent(action: &ToolAction) -> Option<ToolWriteIntent> {
    if !tool_name_is_write_intent(&action.tool_name)
        && !action
            .command
            .as_deref()
            .is_some_and(command_contains_apply_patch_intent)
    {
        return None;
    }
    Some(ToolWriteIntent {
        tool_name: action.tool_name.clone(),
        command: action.command.clone(),
        paths: action.paths.clone(),
    })
}

fn tool_name_is_write_intent(tool_name: &str) -> bool {
    let normalized = tool_name
        .to_ascii_lowercase()
        .chars()
        .map(|character| match character {
            '-' | '/' | ':' => '.',
            _ => character,
        })
        .collect::<String>();
    let leaf = normalized
        .split('.')
        .next_back()
        .unwrap_or(normalized.as_str());
    matches!(
        leaf,
        "edit"
            | "multiedit"
            | "write"
            | "notebookedit"
            | "apply_patch"
            | "applypatch"
            | "fswritefile"
            | "fsremove"
            | "fscopy"
            | "fsrename"
    )
}

fn command_contains_apply_patch_intent(command: &str) -> bool {
    command.contains("apply_patch") || command.contains("*** Begin Patch")
}
