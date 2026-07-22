use std::collections::BTreeMap;

use serde_json::Value;

use crate::protocol::{
    DecisionKind, DecisionSubject, HOOK_DECISION_SCHEMA_ID, HOOK_DECISION_SCHEMA_VERSION,
    HOOK_PROTOCOL_ID, HOOK_PROTOCOL_VERSION, HookDecision, ReasonKind,
};
use crate::tool_action::{ToolAction, collect_tool_actions};

pub struct HookSubagentPermissionContext<'a> {
    pub resident_enabled: bool,
    pub managed_child_name: &'a str,
    pub configured_codex_agent_name: &'a str,
    pub configured_role: &'a str,
    pub codex_hook_agent_id: Option<&'a str>,
    pub codex_hook_agent_type: Option<&'a str>,
    pub resident_child_identity_proof: Option<&'a str>,
    pub resident_child_session_id: Option<&'a str>,
    pub identity_status: &'a str,
    pub sandbox_mode: Option<&'a str>,
    pub session_id: &'a str,
}

impl HookSubagentPermissionContext<'_> {
    /// Authorize a configured resident from stable configuration plus the live
    /// hook identity. `canonicalTarget` is deliberately absent: it is a
    /// dispatch hint, not authorization evidence.
    pub fn resident_authorized(&self) -> bool {
        self.resident_enabled
            && !self.configured_codex_agent_name.trim().is_empty()
            && !self.configured_role.trim().is_empty()
            && self
                .codex_hook_agent_id
                .is_some_and(|agent_id| !agent_id.trim().is_empty())
            && self.codex_hook_agent_type.is_some_and(|live_type| {
                live_type == self.configured_role
                    || (self.configured_codex_agent_name == "asp_explorer"
                        && self.configured_role == "asp_explorer"
                        && live_type == "explorer")
            })
            && self.resident_child_identity_proof == Some("codex-hook-payload-live-target")
            && self.resident_child_session_id == Some(self.session_id)
    }
}

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
    if event != "pre-tool"
        || !context.resident_authorized()
        || !sandbox_mode_is_read_only(context.sandbox_mode)
    {
        return None;
    }
    let intent = payload_write_intent(payload)?;

    let mut fields = BTreeMap::new();
    fields.insert(
        "sessionId".to_string(),
        Value::String(context.session_id.to_string()),
    );
    fields.insert(
        "residentChildName".to_string(),
        Value::String(context.managed_child_name.to_string()),
    );
    fields.insert(
        "readOnlyAgentName".to_string(),
        Value::String(context.configured_codex_agent_name.to_string()),
    );
    fields.insert(
        "readOnlyAgentRole".to_string(),
        Value::String(context.configured_role.to_string()),
    );
    fields.insert(
        "codexHookAgentId".to_string(),
        Value::String(context.codex_hook_agent_id.unwrap_or_default().to_string()),
    );
    fields.insert(
        "codexHookAgentType".to_string(),
        Value::String(
            context
                .codex_hook_agent_type
                .unwrap_or_default()
                .to_string(),
        ),
    );
    fields.insert(
        "residentChildIdentityProof".to_string(),
        Value::String(
            context
                .resident_child_identity_proof
                .unwrap_or_default()
                .to_string(),
        ),
    );
    fields.insert(
        "readOnlySessionId".to_string(),
        Value::String(context.session_id.to_string()),
    );
    fields.insert(
        "residentAuthorizationStatus".to_string(),
        Value::String(context.identity_status.to_string()),
    );
    fields.insert(
        "configuredSandboxMode".to_string(),
        Value::String(context.sandbox_mode.unwrap_or_default().to_string()),
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
            context.session_id, intent.tool_name
        ),
        fields,
    })
}

pub fn classify_read_only_subagent_receipt(
    platform: &str,
    event: &str,
    payload: &Value,
    context: &HookSubagentPermissionContext<'_>,
) -> Option<HookDecision> {
    if event != "subagent-stop"
        || !context.resident_authorized()
        || !sandbox_mode_is_read_only(context.sandbox_mode)
    {
        return None;
    }
    let message = payload_message(payload).unwrap_or_default();
    let validation = validate_asp_search_subagent_receipt(&message);
    if validation.valid {
        return Some(HookDecision {
            schema_id: HOOK_DECISION_SCHEMA_ID,
            schema_version: HOOK_DECISION_SCHEMA_VERSION,
            protocol_id: HOOK_PROTOCOL_ID,
            protocol_version: HOOK_PROTOCOL_VERSION,
            platform: platform.to_string(),
            event: event.to_string(),
            decision: DecisionKind::Allow,
            reason_kind: ReasonKind::None,
            language_ids: Vec::new(),
            subject: DecisionSubject::default(),
            routes: Vec::new(),
            message: "ASP search subagent receipt accepted.".to_string(),
            fields: receipt_fields(context, "accepted", None),
        });
    }

    Some(HookDecision {
        schema_id: HOOK_DECISION_SCHEMA_ID,
        schema_version: HOOK_DECISION_SCHEMA_VERSION,
        protocol_id: HOOK_PROTOCOL_ID,
        protocol_version: HOOK_PROTOCOL_VERSION,
        platform: platform.to_string(),
        event: event.to_string(),
        decision: DecisionKind::Block,
        reason_kind: ReasonKind::SubagentReceiptRequired,
        language_ids: Vec::new(),
        subject: DecisionSubject::default(),
        routes: Vec::new(),
        message: format!(
            "ASP search child `{}` stopped without a valid selector-only graph-route `[asp-search-subagent]` receipt: {}. \
             Re-emit one compact graph-route receipt from the same child session with schema/intent/route/state/evidence/next lines. \
             Do not return source bodies, snippets, line-range selectors, confidence labels, \
             long explanations, or not-found inventories.",
            context.session_id, validation.reason
        ),
        fields: receipt_fields(context, "malformed", Some(validation.reason)),
    })
}

fn sandbox_mode_is_read_only(sandbox_mode: Option<&str>) -> bool {
    sandbox_mode.is_some_and(|mode| {
        let normalized = mode.trim().to_ascii_lowercase();
        normalized == "read-only" || normalized == "readonly"
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

fn string_field(payload: &Value, keys: &[&str]) -> Option<String> {
    keys.iter()
        .find_map(|key| payload.get(*key).and_then(Value::as_str))
        .map(str::to_string)
}

fn payload_message(payload: &Value) -> Option<String> {
    string_field(
        payload,
        &[
            "last_assistant_message",
            "lastAssistantMessage",
            "final_message",
            "finalMessage",
            "message",
            "content",
        ],
    )
}

struct ReceiptValidation {
    valid: bool,
    reason: String,
}

fn validate_asp_search_subagent_receipt(message: &str) -> ReceiptValidation {
    if message.trim().is_empty() {
        return invalid_receipt("empty final message");
    }
    if contains_forbidden_receipt_text(message) {
        return invalid_receipt("receipt contains forbidden prose or confidence/missing/risk text");
    }

    let mut saw_header = false;
    let mut schema = None::<&str>;
    let mut intent = None::<&str>;
    let mut route = None::<&str>;
    let mut state = None::<&str>;
    let mut evidence_count = 0usize;
    let mut next_count = 0usize;
    let mut alt_count = 0usize;
    let mut valid_block = false;
    for line in message
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
    {
        if line == "[asp-search-subagent]" {
            if graph_receipt_block_valid(schema, intent, route, state, evidence_count, next_count) {
                valid_block = true;
            }
            saw_header = true;
            schema = None;
            intent = None;
            route = None;
            state = None;
            evidence_count = 0;
            next_count = 0;
            alt_count = 0;
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            return invalid_receipt("non-receipt line in final message");
        };
        let value = value.trim();
        match key.trim() {
            "schema" => {
                if value != "asp-search-subagent.graph.v1" {
                    return invalid_receipt("unsupported receipt schema");
                }
                schema = Some(value);
            }
            "intent" => match non_empty_value(value, "intent") {
                Ok(value) => intent = value,
                Err(validation) => return validation,
            },
            "route" => match non_empty_value(value, "route") {
                Ok(value) => route = value,
                Err(validation) => return validation,
            },
            "state" => {
                if !matches!(
                    value,
                    "selector-ready"
                        | "needs-refine"
                        | "no-output"
                        | "inventory-only"
                        | "stale-index"
                        | "invalid-selector"
                        | "empty-payload"
                        | "quarantined"
                ) {
                    return invalid_receipt("unsupported graph-route state");
                }
                state = Some(value);
            }
            "evidence" => {
                if let Err(validation) = validate_evidence_row(value) {
                    return validation;
                }
                evidence_count += 1;
            }
            "rankedEvidence" => match validate_ranked_evidence_row(value) {
                Ok(count) => evidence_count += count,
                Err(validation) => return validation,
            },
            "edges" => {
                if let Err(validation) = validate_graph_edges(value) {
                    return validation;
                }
            }
            "next" => {
                if !parent_action_is_safe(value) {
                    return invalid_receipt("next command is not a safe parent ASP action");
                }
                next_count += 1;
            }
            "alt" => {
                if alt_count >= 2 {
                    return invalid_receipt("receipt has too many alternate actions");
                }
                if !parent_action_is_safe(value) {
                    return invalid_receipt("alternate command is not a safe parent ASP action");
                }
                alt_count += 1;
            }
            "avoid" | "omit" => {
                if let Err(validation) = validate_compact_fact_list(value) {
                    return validation;
                }
            }
            _ => return invalid_receipt("unsupported receipt field"),
        }
    }
    if graph_receipt_block_valid(schema, intent, route, state, evidence_count, next_count) {
        valid_block = true;
    }
    if !saw_header {
        return invalid_receipt("missing [asp-search-subagent] receipt header");
    }
    if !valid_block {
        return invalid_receipt(
            "receipt missing graph schema/intent/route/state/evidence/next fields",
        );
    }
    ReceiptValidation {
        valid: true,
        reason: "ok".to_string(),
    }
}

fn invalid_receipt(reason: &str) -> ReceiptValidation {
    ReceiptValidation {
        valid: false,
        reason: reason.to_string(),
    }
}

fn graph_receipt_block_valid(
    schema: Option<&str>,
    intent: Option<&str>,
    route: Option<&str>,
    state: Option<&str>,
    evidence_count: usize,
    next_count: usize,
) -> bool {
    schema == Some("asp-search-subagent.graph.v1")
        && intent.is_some_and(|value| !value.is_empty())
        && route.is_some_and(|value| !value.is_empty())
        && state.is_some_and(|value| !value.is_empty())
        && evidence_count > 0
        && next_count == 1
}

fn non_empty_value<'a>(
    value: &'a str,
    field: &'static str,
) -> Result<Option<&'a str>, ReceiptValidation> {
    if value.is_empty() {
        Err(invalid_receipt(&format!("empty {field} field")))
    } else {
        Ok(Some(value))
    }
}

fn validate_evidence_row(value: &str) -> Result<(), ReceiptValidation> {
    if value.is_empty() {
        return Err(invalid_receipt("empty evidence row"));
    }
    if !value.contains(" kind=") && !value.starts_with("kind=") {
        return Err(invalid_receipt("evidence row missing kind"));
    }
    if !value.contains(" role=") {
        return Err(invalid_receipt("evidence row missing role"));
    }
    if !value.contains(" owner=") {
        return Err(invalid_receipt("evidence row missing owner"));
    }
    let Some(selector) = row_field_value(value, "selector") else {
        return Err(invalid_receipt(
            "evidence row missing parser-owned selector",
        ));
    };
    if selector_is_line_range(selector) {
        return Err(invalid_receipt(
            "evidence selector is a line-range selector",
        ));
    }
    if !selector.contains("://") && !selector.contains('#') {
        return Err(invalid_receipt("evidence selector is not parser-owned"));
    }
    Ok(())
}

fn validate_ranked_evidence_row(value: &str) -> Result<usize, ReceiptValidation> {
    let rows: Vec<_> = value
        .split(';')
        .map(str::trim)
        .filter(|row| !row.is_empty())
        .collect();
    if rows.is_empty() {
        return Err(invalid_receipt("empty ranked evidence row"));
    }
    if rows.len() > 3 {
        return Err(invalid_receipt("ranked evidence has too many rows"));
    }
    for row in &rows {
        validate_evidence_row(row)?;
    }
    Ok(rows.len())
}

fn validate_graph_edges(value: &str) -> Result<(), ReceiptValidation> {
    let edges: Vec<_> = value
        .split(';')
        .map(str::trim)
        .filter(|edge| !edge.is_empty())
        .collect();
    if edges.is_empty() {
        return Err(invalid_receipt("empty graph edge row"));
    }
    if edges.len() > 3 {
        return Err(invalid_receipt("graph receipt has too many edges"));
    }
    for edge in edges {
        let Some((from_and_relation, to)) = edge.split_once("->") else {
            return Err(invalid_receipt("graph edge missing target"));
        };
        let Some((from, relation)) = from_and_relation.split_once('-') else {
            return Err(invalid_receipt("graph edge missing relation"));
        };
        if !receipt_ref_is_valid(from) || !receipt_ref_is_valid(to) || relation.trim().is_empty() {
            return Err(invalid_receipt(
                "graph edge has invalid endpoint or relation",
            ));
        }
    }
    Ok(())
}

fn receipt_ref_is_valid(value: &str) -> bool {
    let value = value.trim();
    value.len() >= 2
        && value.starts_with('E')
        && value[1..]
            .chars()
            .all(|character| character.is_ascii_digit())
}

fn validate_compact_fact_list(value: &str) -> Result<(), ReceiptValidation> {
    if value.is_empty() || value.contains(' ') {
        Err(invalid_receipt("compact fact list must be comma-separated"))
    } else {
        Ok(())
    }
}

fn row_field_value<'a>(row: &'a str, field: &str) -> Option<&'a str> {
    let prefix = format!("{field}=");
    row.split_whitespace()
        .find_map(|token| token.strip_prefix(&prefix))
}

fn contains_forbidden_receipt_text(message: &str) -> bool {
    let normalized = message
        .to_ascii_lowercase()
        .replace("not-found-inventory", "");
    normalized.contains("confidence=")
        || normalized.contains("confidence:")
        || normalized.contains("missing=")
        || normalized.contains("risk=")
        || normalized.contains("not found")
        || normalized.contains("not-found")
        || normalized.contains("```")
}

fn selector_is_line_range(value: &str) -> bool {
    let Some((_, suffix)) = value.rsplit_once(':') else {
        return false;
    };
    if let Some((start, end)) = suffix.split_once('-') {
        return is_decimal(start) && is_decimal(end);
    }
    is_decimal(suffix)
}

fn parent_action_is_safe(value: &str) -> bool {
    let normalized = value.trim();
    if !(normalized.starts_with("asp ") || normalized.contains(" asp ")) {
        return false;
    }
    if normalized.contains("--from-hook item-skeleton") {
        return false;
    }
    if normalized.contains(" query ") {
        return normalized.contains(" --selector ");
    }
    normalized.contains(" search ")
        && normalized.contains(" --workspace ")
        && (normalized.contains(" owner ")
            || normalized.contains(" pipe ")
            || normalized.contains(" deps ")
            || normalized.contains(" lexical "))
}

fn is_decimal(value: &str) -> bool {
    !value.is_empty() && value.bytes().all(|byte| byte.is_ascii_digit())
}

fn receipt_fields(
    context: &HookSubagentPermissionContext<'_>,
    status: &str,
    reason: Option<String>,
) -> BTreeMap<String, Value> {
    let mut fields = BTreeMap::new();
    fields.insert(
        "sessionId".to_string(),
        Value::String(context.session_id.to_string()),
    );
    fields.insert(
        "residentChildName".to_string(),
        Value::String(context.managed_child_name.to_string()),
    );
    fields.insert(
        "readOnlyAgentName".to_string(),
        Value::String(context.configured_codex_agent_name.to_string()),
    );
    fields.insert(
        "readOnlyAgentRole".to_string(),
        Value::String(context.configured_role.to_string()),
    );
    fields.insert(
        "codexHookAgentId".to_string(),
        Value::String(context.codex_hook_agent_id.unwrap_or_default().to_string()),
    );
    fields.insert(
        "codexHookAgentType".to_string(),
        Value::String(
            context
                .codex_hook_agent_type
                .unwrap_or_default()
                .to_string(),
        ),
    );
    fields.insert(
        "residentChildIdentityProof".to_string(),
        Value::String(
            context
                .resident_child_identity_proof
                .unwrap_or_default()
                .to_string(),
        ),
    );
    fields.insert(
        "residentAuthorizationStatus".to_string(),
        Value::String(context.identity_status.to_string()),
    );
    fields.insert(
        "configuredSandboxMode".to_string(),
        Value::String(context.sandbox_mode.unwrap_or_default().to_string()),
    );
    fields.insert(
        "subagentReceiptStatus".to_string(),
        Value::String(status.to_string()),
    );
    fields.insert(
        "requiredAction".to_string(),
        Value::String("return-selector-only-graph-route-receipt".to_string()),
    );
    fields.insert(
        "nextAction".to_string(),
        Value::String("ask-existing-child-to-reemit-graph-route-receipt".to_string()),
    );
    if let Some(reason) = reason {
        fields.insert("receiptError".to_string(), Value::String(reason));
    }
    fields
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
