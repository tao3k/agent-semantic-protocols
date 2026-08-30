//! Shared semantic agent hook protocol models and renderers.

use serde::de;
use serde::{Deserialize, Serialize};

pub(crate) const HOOK_POLICY_KERNEL_VERSION: &str = "1";
use serde_json::{Value, json};
use std::borrow::Cow;
use std::collections::BTreeMap;

/// Schema identifier for semantic hook project activations.
pub const HOOK_ACTIVATION_SCHEMA_ID: &str = "agent.semantic-protocols.hook.activation";
/// Schema version for semantic hook project activations.
pub const HOOK_ACTIVATION_SCHEMA_VERSION: &str = "2";
/// Schema identifier for static semantic hook provider manifests.
pub const PROVIDER_MANIFEST_SCHEMA_ID: &str = "agent.semantic-protocols.hook.provider-manifest";
/// Schema version for static semantic hook provider manifests.
pub const PROVIDER_MANIFEST_SCHEMA_VERSION: &str = "1";
pub const CANONICAL_SCHEMA_AUTHORITY: &str =
    "https://tao3k.github.io/agent-semantic-protocols/schemas/";
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

impl HookPolicy {}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct HookRoutes {
    pub prime: CommandTemplate,
    pub owner: CommandTemplate,
    pub lexical: CommandTemplate,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub query: Option<CommandTemplate>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exact_selector_native: Option<CommandTemplate>,
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
pub struct HookRouteBindings {
    pub prime: String,
    pub owner: String,
    pub lexical: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub query: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exact_selector_native: Option<String>,
    pub ingest: String,
    pub check_changed: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dependency_topology: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dependency_topology_metadata: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub export_index: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub guide: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
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

#[derive(Debug)]
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
    pub language_ids: Vec<agent_semantic_config::LanguageId>,
    pub subject: DecisionSubject,
    pub routes: Vec<DecisionRoute>,
    pub message: String,
    pub fields: BTreeMap<String, Value>,
}

#[path = "protocol_compact_decision.rs"]
mod compact_decision;

impl HookDecision {
    /// Encode an immutable matcher shard without JSON tokenization on the
    /// one-shot reader path.
    pub fn to_compact_binary(&self) -> Result<Vec<u8>, String> {
        compact_decision::encode(self)
    }

    /// Decode the policy compiler's immutable decision shard.
    pub fn from_compact_binary(bytes: &[u8]) -> Result<Self, String> {
        compact_decision::decode(bytes)
    }

    /// Substitute the one source identity minted by the shard compiler.
    pub fn replace_template_marker(&mut self, marker: &str, source_path: &str) -> bool {
        compact_decision::replace_template_marker(self, marker, source_path)
    }
}
impl serde::Serialize for HookDecision {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct SerializedHookDecision<'a> {
            schema_id: &'a str,
            schema_version: &'a str,
            protocol_id: &'a str,
            protocol_version: &'a str,
            platform: &'a str,
            event: &'a str,
            decision: &'a DecisionKind,
            reason_kind: &'a ReasonKind,
            language_ids: &'a [agent_semantic_config::LanguageId],
            subject: &'a DecisionSubject,
            routes: &'a [DecisionRoute],
            message: &'a str,
            #[serde(skip_serializing_if = "BTreeMap::is_empty")]
            fields: &'a BTreeMap<String, Value>,
        }

        SerializedHookDecision {
            schema_id: self.schema_id,
            schema_version: self.schema_version,
            protocol_id: self.protocol_id,
            protocol_version: self.protocol_version,
            platform: &self.platform,
            event: &self.event,
            decision: &self.decision,
            reason_kind: &self.reason_kind,
            language_ids: &self.language_ids,
            subject: &self.subject,
            routes: &self.routes,
            message: &self.message,
            fields: &self.fields,
        }
        .serialize(serializer)
    }
}

impl<'de> serde::Deserialize<'de> for HookDecision {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase", deny_unknown_fields)]
        struct DeserializedHookDecision {
            schema_id: String,
            schema_version: String,
            protocol_id: String,
            protocol_version: String,
            platform: String,
            event: String,
            decision: DecisionKind,
            reason_kind: ReasonKind,
            language_ids: Vec<agent_semantic_config::LanguageId>,
            subject: DecisionSubject,
            routes: Vec<DecisionRoute>,
            message: String,
            #[serde(default)]
            fields: BTreeMap<String, Value>,
        }

        let wire = DeserializedHookDecision::deserialize(deserializer)?;
        validate_hook_decision_identity::<D::Error>(
            "schemaId",
            &wire.schema_id,
            HOOK_DECISION_SCHEMA_ID,
        )?;
        validate_hook_decision_identity::<D::Error>(
            "schemaVersion",
            &wire.schema_version,
            HOOK_DECISION_SCHEMA_VERSION,
        )?;
        validate_hook_decision_identity::<D::Error>(
            "protocolId",
            &wire.protocol_id,
            HOOK_PROTOCOL_ID,
        )?;
        validate_hook_decision_identity::<D::Error>(
            "protocolVersion",
            &wire.protocol_version,
            HOOK_PROTOCOL_VERSION,
        )?;
        Ok(Self {
            schema_id: HOOK_DECISION_SCHEMA_ID,
            schema_version: HOOK_DECISION_SCHEMA_VERSION,
            protocol_id: HOOK_PROTOCOL_ID,
            protocol_version: HOOK_PROTOCOL_VERSION,
            platform: wire.platform,
            event: wire.event,
            decision: wire.decision,
            reason_kind: wire.reason_kind,
            language_ids: wire.language_ids,
            subject: wire.subject,
            routes: wire.routes,
            message: wire.message,
            fields: wire.fields,
        })
    }
}

fn validate_hook_decision_identity<E>(field: &str, actual: &str, expected: &str) -> Result<(), E>
where
    E: de::Error,
{
    if actual == expected {
        Ok(())
    } else {
        Err(E::custom(format!(
            "invalid Hook decision {field}: expected {expected}, found {actual}"
        )))
    }
}

impl HookDecision {
    /// Whether a config rule emitted a complete registered-Agent dispatch.
    pub fn has_registered_agent_dispatch(&self) -> bool {
        self.fields
            .get("agentSessionAction")
            .and_then(Value::as_str)
            == Some("dispatch-registered-agent")
            && self.fields.get("transport").and_then(Value::as_str) == Some("host-agent")
            && ["receiptKind", "targetAgent"].into_iter().all(|field| {
                self.fields
                    .get(field)
                    .and_then(Value::as_str)
                    .is_some_and(|value| !value.is_empty())
            })
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
/// Allow or deny result emitted by the hook classifier.
pub enum DecisionKind {
    Allow,
    Block,
    Deny,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
/// Reason category for a hook decision.
pub enum ReasonKind {
    None,
    HostActionAuthorityUnavailable,
    ActivationUnavailable,
    RegisteredSourceRouteRequired,
    StructuredSourceRead,
    BulkSourceDump,
    RawBroadSearch,
    AspReasoningRouted,
    SourceDirectoryEnumeration,
    AgentSearchJson,
    SemanticAstPatchRequired,
    AgentChoiceRequired,
    FocusedSubagentNestedStart,
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
    if !deserializer.is_human_readable() {
        return Option::<StdinMode>::deserialize(deserializer);
    }
    let value = Value::deserialize(deserializer)?;
    if value.is_null() {
        return Err(de::Error::custom("stdinMode must be omitted, not null"));
    }
    serde_json::from_value(value)
        .map(Some)
        .map_err(de::Error::custom)
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
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

#[derive(Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
/// Tool name, command, and paths that triggered a hook decision.
pub struct DecisionSubject {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub command: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub paths: Vec<String>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
/// Provider command route that the agent should run instead of denied tool use.
pub struct DecisionRoute {
    pub language_id: agent_semantic_config::LanguageId,
    pub provider_id: agent_semantic_config::ProviderId,
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

impl AgentHookError {
    #[must_use]
    pub const fn reason_kind(&self) -> &'static str {
        match self {
            Self::InvalidActivation(_) => "invalid-activation",
            Self::InvalidActivationConfig(_) => "invalid-activation-config",
            Self::InvalidPayload(_) => "invalid-payload",
            Self::InvalidOutput(_) => "invalid-output",
        }
    }

    #[must_use]
    pub fn message(&self) -> String {
        match self {
            Self::InvalidActivation(error) => format!("invalid activation JSON: {error}"),
            Self::InvalidActivationConfig(message) => message.clone(),
            Self::InvalidPayload(error) => format!("invalid hook payload JSON: {error}"),
            Self::InvalidOutput(error) => format!("invalid hook output JSON: {error}"),
        }
    }
}

impl std::fmt::Display for AgentHookError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{}: {}", self.reason_kind(), self.message())
    }
}

impl std::error::Error for AgentHookError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::InvalidActivation(error)
            | Self::InvalidPayload(error)
            | Self::InvalidOutput(error) => Some(error),
            Self::InvalidActivationConfig(_) => None,
        }
    }
}

/// Parse a platform hook payload as JSON.
pub fn parse_payload(input: &str) -> Result<Value, AgentHookError> {
    serde_json::from_str(input).map_err(AgentHookError::InvalidPayload)
}

/// Render the only Codex PreToolUse deny wire shape emitted by ASP.
///
/// The typed decision is persisted in ASP's event ledger. The Host envelope
/// deliberately exposes only the configured natural-language instruction:
/// models should use the native collaboration operation, not parse an
/// internal receipt.
pub fn render_codex_pre_tool_deny(_decision_value: &Value, message: &str) -> Value {
    json!({
        "hookSpecificOutput": {
            "hookEventName": "PreToolUse",
            "permissionDecision": "deny",
            "permissionDecisionReason": message,
            "additionalContext": message,
        },
        "systemMessage": message,
    })
}

/// Render a Codex PermissionRequest decision without leaking ASP receipt
/// fields into the Host schema.
pub fn render_codex_permission_request(behavior: &str, message: Option<&str>) -> Value {
    let mut decision = json!({ "behavior": behavior });
    if let Some(message) = message {
        decision["message"] = Value::String(message.to_owned());
    }
    let mut envelope = json!({
        "hookSpecificOutput": {
            "hookEventName": "PermissionRequest",
            "decision": decision,
        }
    });
    if let Some(message) = message {
        envelope["systemMessage"] = Value::String(message.to_owned());
    }
    envelope
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
    let decision_context = message.as_ref().to_owned();
    if decision.platform == "codex"
        && decision.event == "post-tool"
        && !matches!(decision.decision, DecisionKind::Allow)
    {
        return Ok(json!({
            "hookSpecificOutput": {
                "hookEventName": "PostToolUse",
                "additionalContext": decision_context,
            },
            "systemMessage": message.as_ref(),
        }));
    }
    match decision.decision {
        DecisionKind::Deny => {
            if decision.platform == "codex" && decision.event == "permission-request" {
                return Ok(render_codex_permission_request(
                    "deny",
                    Some(message.as_ref()),
                ));
            }
            if decision.platform == "codex" && decision.event == "pre-tool" {
                return Ok(render_codex_pre_tool_deny(
                    &decision_value,
                    message.as_ref(),
                ));
            }
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
                if decision.platform == "codex" {
                    return Ok(render_codex_permission_request("allow", None));
                }
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
        || decision.fields.contains_key("bootstrapReceipt")
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
        if line.contains("collaboration.spawn_agent")
            || line.contains("collaboration.followup_task")
            || line.contains("collaboration.send_message")
        {
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
        return "ASP evidence-state search routing is active for this prompt. This is a locator/frontier question: answer where to look before editing, not by reading source code. Search is not a mandatory pipeline. Choose the narrowest ASP route whose preconditions are already satisfied. If an exact selector, owner path, symbol, dependency, test/failure, changed file, or previous recommendedNext exists, use that anchor and skip `search prime`. Use `search prime --workspace <workspace-root> --view seeds` only when the workspace, project, or owner map is unknown. Use `search pipe '<question-or-feature-term>' --workspace <workspace-root> --view seeds` only when the evidence state is ambiguous and needs query refinement. Do not answer from prime alone; prime is only a project map and is never final evidence. Do not repeat an exact ASP command. Use owner/frontier/locator metadata from search output. Subagents should return one compact `[asp-search-subagent]` graph-route receipt with schema/intent/route/state/evidence/next, never source bodies or line-range selectors. Materialize source only from an exact structural selector with `asp <language> query --selector <exact-selector> --workspace . --projection source`; use `--projection callable-skeleton` for a callable skeleton. Exact query has no implicit projection. ASP facades are language IDs, not package names; for Effect use `asp typescript ...`.";
    }
    "ASP evidence-state search routing is active for this prompt. Before reading source or running raw grep/find, use parser-owned ASP discovery. Search is not a mandatory pipeline. Choose the narrowest ASP route whose preconditions are already satisfied. If an exact selector, owner path, symbol, dependency, test/failure, changed file, or previous recommendedNext exists, use that anchor and skip `search prime`. Use `search prime --workspace <workspace-root> --view seeds` only when the workspace, project, or owner map is unknown. Use `search pipe '<question-or-feature-term>' --workspace <workspace-root> --view seeds` only when the evidence state is ambiguous and needs query refinement. Do not answer from prime alone; prime is only a project map and is never final evidence. Do not repeat an exact ASP command. Follow `recommendedNext` or `nextCommand` from ASP output. Subagents perform owner/frontier/search work and return one compact `[asp-search-subagent]` graph-route receipt with schema/intent/route/state/evidence/next, never source bodies or line-range selectors. After ASP provides exact parser-owned identity, use `asp <language> query --selector <exact-selector> --workspace . --projection source`, or `--projection callable-skeleton` for a callable skeleton. Exact query has no implicit projection. Treat display line ranges and sourceLocatorHint as hints, not executable selectors. Do not use direct source reads as the first step. ASP facades are language IDs, not package names; for Effect use `asp typescript ...`."
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
