//! Shared semantic agent hook protocol models and renderers.

use serde::de;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::borrow::Cow;
use std::collections::BTreeMap;

/// Schema identifier for semantic hook project activations.
pub const HOOK_ACTIVATION_SCHEMA_ID: &str = "agent.semantic-protocols.hook.activation";
/// Schema version for semantic hook project activations.
pub const HOOK_ACTIVATION_SCHEMA_VERSION: &str = "1";
/// Schema identifier for static semantic hook provider manifests.
pub const PROVIDER_MANIFEST_SCHEMA_ID: &str = "agent.semantic-protocols.hook.provider-manifest";
/// Schema version for static semantic hook provider manifests.
pub const PROVIDER_MANIFEST_SCHEMA_VERSION: &str = "1";
/// Schema identifier for shared hook decision packets.
pub const HOOK_DECISION_SCHEMA_ID: &str = "agent.semantic-protocols.hook.decision";
/// Schema version for shared hook decision packets.
pub const HOOK_DECISION_SCHEMA_VERSION: &str = "1";
/// Protocol identifier for the root semantic hook runtime.
pub const HOOK_PROTOCOL_ID: &str = "agent.semantic-protocols.hook";
/// Protocol version for the root semantic hook runtime.
pub const HOOK_PROTOCOL_VERSION: &str = "1";

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
/// Policy switches that control how the root hook classifier handles a provider.
pub struct HookPolicy {
    pub direct_source_read: ActionPolicy,
    pub bulk_source_dump: ActionPolicy,
    pub raw_source_search: ActionPolicy,
    pub agent_search_json: ActionPolicy,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
/// Per-action enforcement mode for hook policy.
pub enum ActionPolicy {
    Block,
    Allow,
    Advisory,
}

impl Default for HookPolicy {
    fn default() -> Self {
        Self {
            direct_source_read: ActionPolicy::Block,
            bulk_source_dump: ActionPolicy::Block,
            raw_source_search: ActionPolicy::Block,
            agent_search_json: ActionPolicy::Block,
        }
    }
}

impl HookPolicy {
    pub(crate) fn blocks_direct_source_read(&self) -> bool {
        action_blocks(self.direct_source_read)
    }

    pub(crate) fn blocks_bulk_source_dump(&self) -> bool {
        action_blocks(self.bulk_source_dump)
    }

    pub(crate) fn blocks_raw_source_search(&self) -> bool {
        action_blocks(self.raw_source_search)
    }

    pub(crate) fn blocks_agent_search_json(&self) -> bool {
        action_blocks(self.agent_search_json)
    }
}

fn action_blocks(action: ActionPolicy) -> bool {
    match action {
        ActionPolicy::Block => true,
        ActionPolicy::Allow | ActionPolicy::Advisory => false,
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
/// Command templates that route denied tool use into semantic search.
pub struct HookRoutes {
    pub prime: CommandTemplate,
    pub owner: CommandTemplate,
    pub lexical: CommandTemplate,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub query: Option<CommandTemplate>,
    pub ingest: CommandTemplate,
    pub check_changed: CommandTemplate,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dependency_topology: Option<CommandTemplate>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dependency_topology_metadata: Option<CommandTemplate>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub export_index: Option<CommandTemplate>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub guide: Option<CommandTemplate>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
/// Argument template for a provider-owned semantic search command.
pub struct CommandTemplate {
    pub argv: Vec<String>,
    #[serde(
        default,
        deserialize_with = "deserialize_optional_stdin_mode",
        skip_serializing_if = "Option::is_none"
    )]
    pub stdin_mode: Option<StdinMode>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
/// Shared decision packet emitted before platform-specific hook rendering.
pub struct HookDecision {
    pub schema_id: &'static str,
    pub schema_version: &'static str,
    pub protocol_id: &'static str,
    pub protocol_version: &'static str,
    pub platform: String,
    pub event: String,
    pub decision: DecisionKind,
    pub reason_kind: ReasonKind,
    pub language_ids: Vec<String>,
    pub subject: DecisionSubject,
    pub routes: Vec<DecisionRoute>,
    pub message: String,
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub fields: BTreeMap<String, Value>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
/// Allow or deny result emitted by the hook classifier.
pub enum DecisionKind {
    Allow,
    Block,
    Deny,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
/// Reason category for a hook decision.
pub enum ReasonKind {
    None,
    DirectSourceRead,
    BulkSourceDump,
    RawBroadSearch,
    SourceDirectoryEnumeration,
    AgentSearchJson,
    SemanticAstPatchRequired,
    ReadOnlySubagentWrite,
    SubagentReceiptRequired,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
/// Stdin handling contract for a semantic hook route.
pub enum StdinMode {
    None,
    PipeCandidates,
    PipeDiff,
    Unknown,
}

fn deserialize_optional_stdin_mode<'de, D>(deserializer: D) -> Result<Option<StdinMode>, D::Error>
where
    D: de::Deserializer<'de>,
{
    let value = Value::deserialize(deserializer)?;
    if value.is_null() {
        return Err(de::Error::custom("stdinMode must be omitted, not null"));
    }
    serde_json::from_value(value)
        .map(Some)
        .map_err(de::Error::custom)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
/// Semantic route kind suggested by a hook denial.
pub enum DecisionRouteKind {
    Prime,
    Owner,
    Query,
    Lexical,
    Read,
    Deps,
    Api,
    Ingest,
    Tests,
    CheckChanged,
}

#[derive(Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
/// Tool name, command, and paths that triggered a hook decision.
pub struct DecisionSubject {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub command: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub paths: Vec<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
/// Provider command route that the agent should run instead of denied tool use.
pub struct DecisionRoute {
    pub language_id: String,
    pub provider_id: String,
    pub binary: String,
    pub kind: DecisionRouteKind,
    pub argv: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stdin_mode: Option<StdinMode>,
}

#[derive(Debug)]
/// Errors produced while parsing activation, payloads, or rendered hook output.
pub enum AgentHookError {
    InvalidActivation(serde_json::Error),
    InvalidActivationConfig(String),
    InvalidPayload(serde_json::Error),
    InvalidOutput(serde_json::Error),
}

/// Parse a platform hook payload as JSON.
pub fn parse_payload(input: &str) -> Result<Value, AgentHookError> {
    serde_json::from_str(input).map_err(AgentHookError::InvalidPayload)
}

/// Render a shared hook decision into the selected platform response envelope.
pub fn render_platform_response(decision: &HookDecision) -> Result<Value, AgentHookError> {
    let message = platform_decision_message(decision);
    let mut decision_value =
        serde_json::to_value(decision).map_err(AgentHookError::InvalidOutput)?;
    if message.as_ref() != decision.message
        && let Some(object) = decision_value.as_object_mut()
    {
        object.insert(
            "message".to_string(),
            Value::String(message.as_ref().to_string()),
        );
    }
    let decision_context = format!(
        "[agent-hook-decision] {}",
        serde_json::to_string(&decision_value).map_err(AgentHookError::InvalidOutput)?
    );
    match decision.decision {
        DecisionKind::Deny => {
            return Ok(json!({
                "hookSpecificOutput": {
                    "hookEventName": platform_hook_event_name(&decision.event),
                    "permissionDecision": "deny",
                    "permissionDecisionReason": message.as_ref(),
                    "additionalContext": decision_context,
                },
                "systemMessage": message.as_ref(),
            }));
        }
        DecisionKind::Block => {
            let additional_context = if decision.event == "stop" {
                format!("{decision_context}\n\n{}", message.as_ref())
            } else {
                decision_context
            };
            return Ok(json!({
                "decision": "block",
                "reason": message.as_ref(),
                "hookSpecificOutput": {
                    "hookEventName": platform_hook_event_name(&decision.event),
                    "additionalContext": additional_context,
                },
                "systemMessage": message.as_ref(),
            }));
        }
        DecisionKind::Allow => {
            if decision.event == "permission-request" {
                return Ok(json!({
                    "hookSpecificOutput": {
                        "hookEventName": platform_hook_event_name(&decision.event),
                        "permissionDecision": "allow",
                        "additionalContext": decision_context,
                    }
                }));
            }
            if decision.event == "user-prompt" {
                let locator_only = decision
                    .fields
                    .get("promptWorkflow")
                    .and_then(|value| value.as_str())
                    == Some("locator-only");
                return Ok(json!({
                    "hookSpecificOutput": {
                        "hookEventName": platform_hook_event_name(&decision.event),
                        "additionalContext": user_prompt_search_first_context(locator_only),
                    }
                }));
            }
            if decision_has_warning(decision) {
                return Ok(json!({
                    "hookSpecificOutput": {
                        "hookEventName": platform_hook_event_name(&decision.event),
                        "additionalContext": decision_context,
                    },
                    "systemMessage": message.as_ref(),
                }));
            }
        }
    }
    Ok(json!({}))
}

fn decision_has_warning(decision: &HookDecision) -> bool {
    decision
        .fields
        .contains_key("agentOrgArtifactsArchiveWarning")
}

fn platform_decision_message(decision: &HookDecision) -> Cow<'_, str> {
    if decision.decision == DecisionKind::Deny && is_subagent_context(decision) {
        Cow::Owned(subagent_deny_message(&decision.message))
    } else {
        Cow::Borrowed(&decision.message)
    }
}

fn is_subagent_context(decision: &HookDecision) -> bool {
    ["subagentContext", "isSubagent", "subagent"]
        .iter()
        .any(|field| decision.fields.get(*field).and_then(Value::as_bool) == Some(true))
}

pub fn subagent_deny_message(message: &str) -> String {
    let mut lines = Vec::new();
    let mut inserted_subagent_instruction = false;
    for line in message.lines() {
        if line.contains("spawn_agent") || line.contains("send_input") {
            if !inserted_subagent_instruction {
                lines.push(
                    "Codex: already running inside a subagent; run the safe route below directly and return selector-only `[asp-search-subagent]` evidence with owner/read/next. Do not return source bodies, snippets, or line-range selectors.",
                );
                inserted_subagent_instruction = true;
            }
            continue;
        }
        if line.starts_with("If subagents are unavailable")
            || line.starts_with("No subagent available?")
        {
            continue;
        }
        lines.push(line);
    }
    lines.join("\n")
}

fn user_prompt_search_first_context(locator_only: bool) -> &'static str {
    if locator_only {
        return "ASP evidence-state search routing is active for this prompt. This is a locator/frontier question: answer where to look before editing, not by reading source code. Search is not a mandatory pipeline. Choose the narrowest ASP route whose preconditions are already satisfied. If an exact selector, owner path, symbol, dependency, test/failure, changed file, or previous recommendedNext exists, use that anchor and skip `search prime`. Use `search prime --workspace <workspace-root> --view seeds` only when the workspace, project, or owner map is unknown. Use `search pipe '<question-or-feature-term>' --workspace <workspace-root> --view seeds` only when the evidence state is ambiguous and needs query refinement. Do not answer from prime alone; prime is only a project map and is never final evidence. Do not repeat an exact ASP command. Use owner/frontier/locator metadata from search output. Subagents should return one compact `[asp-search-subagent]` graph-route receipt with schema/intent/route/state/evidence/next, never source bodies or line-range selectors. Do not run `query --code` unless the user explicitly asks for code contents. ASP facades are language IDs, not package names; for Effect use `asp typescript ...`.";
    }
    "ASP evidence-state search routing is active for this prompt. Before reading source or running raw grep/find, use parser-owned ASP discovery. Search is not a mandatory pipeline. Choose the narrowest ASP route whose preconditions are already satisfied. If an exact selector, owner path, symbol, dependency, test/failure, changed file, or previous recommendedNext exists, use that anchor and skip `search prime`. Use `search prime --workspace <workspace-root> --view seeds` only when the workspace, project, or owner map is unknown. Use `search pipe '<question-or-feature-term>' --workspace <workspace-root> --view seeds` only when the evidence state is ambiguous and needs query refinement. Do not answer from prime alone; prime is only a project map and is never final evidence. Do not repeat an exact ASP command. Follow `recommendedNext` or `nextCommand` from ASP output. Subagents perform owner/frontier/search work and return one compact `[asp-search-subagent]` graph-route receipt with schema/intent/route/state/evidence/next, never source bodies or line-range selectors. Use one `asp <language> query --selector <exact-selector> --workspace . --code` only after ASP provides exact parser-owned identity, then answer from that selector plus search metadata. Treat display line ranges and sourceLocatorHint as hints, not executable selectors. Do not use direct source reads as the first step. ASP facades are language IDs, not package names; for Effect use `asp typescript ...`."
}

pub(crate) fn normalize_source_selector(selector: &str) -> &str {
    strip_line_locator(normalize_source_route_selector(selector))
}

pub(crate) fn normalize_source_route_selector(selector: &str) -> &str {
    let selector = selector.trim_start_matches("./");
    let selector = strip_display_rank_prefix(selector);
    strip_display_line_locator(selector)
}

fn strip_display_rank_prefix(selector: &str) -> &str {
    let Some((rank, path)) = selector.split_once(':') else {
        return selector;
    };
    if rank.is_empty()
        || path.is_empty()
        || !rank.chars().all(|character| character.is_ascii_digit())
    {
        return selector;
    }
    path
}

fn strip_display_line_locator(selector: &str) -> &str {
    let Some((path, locator)) = selector.rsplit_once(':') else {
        return selector;
    };
    if is_display_line_range_locator(locator) {
        return path;
    }
    selector
}

fn is_display_line_range_locator(locator: &str) -> bool {
    let Some((start, end)) = locator.split_once('\u{2013}') else {
        return false;
    };
    is_decimal_locator(start) && is_decimal_locator(end)
}

fn platform_hook_event_name(event: &str) -> &'static str {
    match event {
        "session-start" => "SessionStart",
        "user-prompt" => "UserPromptSubmit",
        "pre-tool" => "PreToolUse",
        "permission-request" => "PermissionRequest",
        "post-tool" => "PostToolUse",
        "subagent-start" => "SubagentStart",
        "subagent-stop" => "SubagentStop",
        "stop" => "Stop",
        _ => "Unknown",
    }
}
fn strip_line_locator(selector: &str) -> &str {
    let Some((path, suffix)) = selector.rsplit_once(':') else {
        return selector;
    };
    if is_line_range_locator(suffix) {
        path
    } else {
        selector
    }
}

fn is_line_range_locator(value: &str) -> bool {
    if let Some((start, end)) = value.split_once('-') {
        is_decimal_locator(start) && is_decimal_locator(end)
    } else {
        is_decimal_locator(value)
    }
}

fn is_decimal_locator(value: &str) -> bool {
    !value.is_empty() && value.bytes().all(|byte| byte.is_ascii_digit())
}
