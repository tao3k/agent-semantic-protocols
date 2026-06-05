//! Optional client-side hook rules loaded on each hook invocation.

use figment::{
    Figment,
    providers::{Format, Toml},
};
use globset::{GlobBuilder, GlobSet, GlobSetBuilder};
use serde::Deserialize;
use std::{
    collections::HashSet,
    path::{Path, PathBuf},
};

use crate::command::semantic_shell_tokens;
use crate::protocol::{
    DecisionKind, DecisionRoute, DecisionRouteKind, HOOK_DECISION_SCHEMA_ID,
    HOOK_DECISION_SCHEMA_VERSION, HOOK_PROTOCOL_ID, HOOK_PROTOCOL_VERSION, HookDecision,
    ReasonKind, StdinMode,
};
use crate::protocol_activation::HookRuntime;
use crate::source_selector::collect_source_selector_matches;
use crate::tool_action::{ToolAction, subject_for_action};

const CLIENT_HOOK_CONFIG_SCHEMA_ID: &str = "agent.semantic-protocols.hook.client-config";
const CLIENT_HOOK_CONFIG_SCHEMA_VERSION: &str = "1";

#[derive(Debug, Default)]
/// Compiled project-local hook rules loaded from `.codex/agent-semantic-protocol`.
pub struct ClientHookConfig {
    rules: Vec<CompiledHookRule>,
    semantic_ast_patch_disabled: bool,
}

#[derive(Debug)]
struct CompiledHookRule {
    id: String,
    priority: i64,
    decision: ConfigDecision,
    reason_kind: ReasonKind,
    message: Option<String>,
    language_ids: Vec<String>,
    event: Option<String>,
    platform: Option<String>,
    match_config: RuleMatch,
    routes: Vec<RuleRoute>,
}

#[derive(Debug)]
struct RuleMatch {
    tool_any: Vec<String>,
    command_any: Vec<String>,
    command_contains_any: Vec<String>,
    path_any: Vec<String>,
    path_glob_any: Option<GlobSet>,
}

#[derive(Debug)]
struct RuleRoute {
    provider_id: String,
    language_id: Option<String>,
    binary: Option<String>,
    kind: DecisionRouteKind,
    argv: Vec<String>,
    stdin_mode: Option<StdinMode>,
}

#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
enum ConfigDecision {
    Block,
    Deny,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ConfigFile {
    #[serde(default)]
    schema_id: Option<String>,
    #[serde(default)]
    schema_version: Option<String>,
    #[serde(default)]
    protocol_id: Option<String>,
    #[serde(default)]
    protocol_version: Option<String>,
    #[serde(default)]
    experimental: std::collections::BTreeMap<String, std::collections::BTreeMap<String, bool>>,
    #[serde(default)]
    rules: Vec<RuleConfig>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RuleConfig {
    id: String,
    #[serde(default = "default_enabled")]
    enabled: bool,
    #[serde(default)]
    priority: i64,
    decision: ConfigDecision,
    #[serde(default)]
    reason_kind: Option<String>,
    #[serde(default)]
    message: Option<String>,
    #[serde(default)]
    language_ids: Vec<String>,
    #[serde(default)]
    event: Option<String>,
    #[serde(default)]
    platform: Option<String>,
    #[serde(default, rename = "match")]
    match_config: RuleMatchConfig,
    #[serde(default)]
    routes: Vec<RuleRouteConfig>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RuleMatchConfig {
    #[serde(default)]
    tool: Option<String>,
    #[serde(default)]
    tool_any: Vec<String>,
    #[serde(default)]
    command_any: Vec<String>,
    #[serde(default)]
    command_contains_any: Vec<String>,
    #[serde(default)]
    path_any: Vec<String>,
    #[serde(default)]
    path_glob_any: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RuleRouteConfig {
    provider_id: String,
    #[serde(default)]
    language_id: Option<String>,
    #[serde(default)]
    binary: Option<String>,
    kind: String,
    argv: Vec<String>,
    #[serde(default)]
    stdin_mode: Option<StdinMode>,
}

/// Return the versioned project-local hook config path.
pub fn default_client_config_path(project_root: &str) -> PathBuf {
    PathBuf::from(project_root)
        .join(".codex")
        .join("agent-semantic-protocol")
        .join("hooks")
        .join("config.toml")
}

/// Render the seed project-local hook config file.
pub fn default_client_config_template() -> String {
    format!(
        r#"# Semantic agent client hook config.
# Loaded by `asp hook` on every client hook invocation.
# Missing or comment-only rules preserve the built-in classifier behavior.
schemaId = "{CLIENT_HOOK_CONFIG_SCHEMA_ID}"
schemaVersion = "{CLIENT_HOOK_CONFIG_SCHEMA_VERSION}"
protocolId = "{HOOK_PROTOCOL_ID}"
protocolVersion = "{HOOK_PROTOCOL_VERSION}"

# AST patch routing is experimental and disabled for generated Codex hook configs.
[experimental.semanticAstPatch]
enabled = false

# Uncomment and edit this example to add a project-local rule.
# [[rules]]
# id = "deny-rust-raw-search"
# enabled = true
# event = "pre-tool"
# priority = 100
# decision = "deny"
# reasonKind = "raw-broad-search"
# languageIds = ["rust"]
# message = "Use asp rust search ingest instead of raw Rust source search."
#
# [rules.match]
# tool = "Bash"
# commandAny = ["rg", "grep", "fd", "find"]
# pathGlobAny = ["**/*.rs"]
#
# [[rules.routes]]
# providerId = "rs-harness"
# languageId = "rust"
# binary = "asp"
# kind = "ingest"
# argv = ["asp", "rust", "search", "ingest", "items", "tests", "--view", "seeds", "."]
# stdinMode = "pipe-candidates"
"#
    )
}

/// Load and compile project-local hook config rules.
pub fn load_client_config(path: &Path) -> Result<ClientHookConfig, String> {
    if !path.is_file() {
        return Ok(ClientHookConfig::default());
    }
    let parsed = Figment::from(Toml::file(path))
        .extract::<ConfigFile>()
        .map_err(|error| format!("failed to parse {}: {error}", path.display()))?;
    compile_config(parsed)
}

impl ClientHookConfig {
    pub(crate) fn semantic_ast_patch_enabled(&self) -> bool {
        !self.semantic_ast_patch_disabled
    }

    pub(crate) fn classify(
        &self,
        runtime: &HookRuntime,
        platform: &str,
        event: &str,
        action: &ToolAction,
    ) -> Option<HookDecision> {
        self.rules
            .iter()
            .find(|rule| rule.matches(runtime, platform, event, action))
            .map(|rule| rule.decision(runtime, platform, event, action))
    }
}

impl CompiledHookRule {
    fn matches(
        &self,
        runtime: &HookRuntime,
        platform: &str,
        event: &str,
        action: &ToolAction,
    ) -> bool {
        self.platform
            .as_deref()
            .is_none_or(|expected| expected.eq_ignore_ascii_case(platform))
            && self
                .event
                .as_deref()
                .is_none_or(|expected| canonical_event(expected) == canonical_event(event))
            && self.matches_language(runtime, action)
            && self.match_config.matches(action)
    }

    fn matches_language(&self, runtime: &HookRuntime, action: &ToolAction) -> bool {
        if self.language_ids.is_empty() {
            return true;
        }
        if action.paths.is_empty() {
            return false;
        }
        !collect_source_selector_matches(
            runtime,
            action.paths.iter().map(String::as_str),
            |provider| {
                self.language_ids
                    .iter()
                    .any(|language_id| language_id.eq_ignore_ascii_case(&provider.language_id))
            },
        )
        .is_empty()
    }

    fn decision(
        &self,
        runtime: &HookRuntime,
        platform: &str,
        event: &str,
        action: &ToolAction,
    ) -> HookDecision {
        let decision = match self.decision {
            ConfigDecision::Block => DecisionKind::Block,
            ConfigDecision::Deny => DecisionKind::Deny,
        };
        let routes = self
            .routes
            .iter()
            .map(|route| route.decision_route(runtime))
            .collect::<Vec<_>>();
        let message = self.message.clone().unwrap_or_else(|| {
            format!(
                "client hook config rule `{}` matched this tool use",
                self.id
            )
        });
        HookDecision {
            schema_id: HOOK_DECISION_SCHEMA_ID,
            schema_version: HOOK_DECISION_SCHEMA_VERSION,
            protocol_id: HOOK_PROTOCOL_ID,
            protocol_version: HOOK_PROTOCOL_VERSION,
            platform: platform.to_string(),
            event: event.to_string(),
            decision,
            reason_kind: self.reason_kind,
            language_ids: self.language_ids.clone(),
            subject: subject_for_action(action),
            routes,
            message,
            fields: std::collections::BTreeMap::from([(
                "configRuleId".to_string(),
                serde_json::Value::String(self.id.clone()),
            )]),
        }
    }
}

impl RuleMatch {
    fn matches(&self, action: &ToolAction) -> bool {
        self.matches_tool(action) && self.matches_command(action) && self.matches_path(action)
    }

    fn matches_tool(&self, action: &ToolAction) -> bool {
        self.tool_any.is_empty()
            || self
                .tool_any
                .iter()
                .any(|tool| tool.eq_ignore_ascii_case(&action.tool_name))
    }

    fn matches_command(&self, action: &ToolAction) -> bool {
        let Some(command) = action.command.as_deref() else {
            return self.command_any.is_empty() && self.command_contains_any.is_empty();
        };
        let tokens = semantic_shell_tokens(command);
        let token_match = self.command_any.is_empty()
            || self.command_any.iter().any(|expected| {
                tokens
                    .iter()
                    .any(|token| token.eq_ignore_ascii_case(expected))
            });
        let contains_match = self.command_contains_any.is_empty()
            || self
                .command_contains_any
                .iter()
                .any(|expected| command.contains(expected));
        token_match && contains_match
    }

    fn matches_path(&self, action: &ToolAction) -> bool {
        if self.path_any.is_empty() && self.path_glob_any.is_none() {
            return true;
        }
        let exact_match = !self.path_any.is_empty()
            && action.paths.iter().any(|path| {
                self.path_any
                    .iter()
                    .any(|expected| path == expected || path.ends_with(expected))
            });
        let glob_match = self.path_glob_any.as_ref().is_some_and(|globset| {
            action
                .paths
                .iter()
                .any(|path| globset.is_match(path.as_str()))
        });
        exact_match || glob_match
    }
}

impl RuleRoute {
    fn decision_route(&self, runtime: &HookRuntime) -> DecisionRoute {
        let provider = runtime
            .providers
            .iter()
            .find(|provider| provider.provider_id == self.provider_id);
        DecisionRoute {
            language_id: self
                .language_id
                .clone()
                .or_else(|| provider.map(|provider| provider.language_id.clone()))
                .unwrap_or_default(),
            provider_id: self.provider_id.clone(),
            binary: self
                .binary
                .clone()
                .or_else(|| provider.map(|provider| provider.binary.clone()))
                .unwrap_or_default(),
            kind: self.kind,
            argv: self.argv.clone(),
            stdin_mode: self.stdin_mode,
        }
    }
}

fn compile_config(config: ConfigFile) -> Result<ClientHookConfig, String> {
    config.validate_protocol()?;
    validate_unique_rule_ids(&config.rules)?;
    validate_rule_schema_shape(&config.rules)?;
    let semantic_ast_patch_enabled = config
        .experimental
        .get("semanticAstPatch")
        .and_then(|feature| feature.get("enabled"))
        .copied()
        .unwrap_or(true);
    let mut rules = config
        .rules
        .into_iter()
        .filter(|rule| rule.enabled)
        .map(CompiledHookRule::try_from)
        .collect::<Result<Vec<_>, _>>()?;
    // `sort_by` is stable, so equal-priority rules keep config file order.
    rules.sort_by(|left, right| right.priority.cmp(&left.priority));
    Ok(ClientHookConfig {
        rules,
        semantic_ast_patch_disabled: !semantic_ast_patch_enabled,
    })
}

fn validate_unique_rule_ids(rules: &[RuleConfig]) -> Result<(), String> {
    let mut seen = HashSet::new();
    for rule in rules {
        if !seen.insert(rule.id.as_str()) {
            return Err(format!("duplicate client hook rule id `{}`", rule.id));
        }
    }
    Ok(())
}

fn validate_rule_schema_shape(rules: &[RuleConfig]) -> Result<(), String> {
    for rule in rules {
        validate_identifier("rules[].id", &rule.id)?;
        validate_optional_non_empty("rules[].message", rule.message.as_deref())?;
        validate_optional_event(rule.event.as_deref())?;
        validate_optional_platform(rule.platform.as_deref())?;
        validate_unique_values("rules[].languageIds", &rule.language_ids)?;
        validate_identifiers("rules[].languageIds[]", &rule.language_ids)?;
        validate_match_schema_shape(&rule.match_config)?;
        for route in &rule.routes {
            validate_route_schema_shape(route)?;
        }
    }
    Ok(())
}

fn validate_match_schema_shape(match_config: &RuleMatchConfig) -> Result<(), String> {
    validate_optional_non_empty("rules[].match.tool", match_config.tool.as_deref())?;
    validate_non_empty_values("rules[].match.toolAny[]", &match_config.tool_any)?;
    validate_non_empty_values("rules[].match.commandAny[]", &match_config.command_any)?;
    validate_non_empty_values(
        "rules[].match.commandContainsAny[]",
        &match_config.command_contains_any,
    )?;
    validate_non_empty_values("rules[].match.pathAny[]", &match_config.path_any)?;
    validate_non_empty_values("rules[].match.pathGlobAny[]", &match_config.path_glob_any)?;
    Ok(())
}

fn validate_route_schema_shape(route: &RuleRouteConfig) -> Result<(), String> {
    validate_identifier("rules[].routes[].providerId", &route.provider_id)?;
    if let Some(language_id) = &route.language_id {
        validate_identifier("rules[].routes[].languageId", language_id)?;
    }
    if let Some(binary) = &route.binary {
        validate_binary_name("rules[].routes[].binary", binary)?;
    }
    if route.argv.is_empty() {
        return Err("rules[].routes[].argv must contain at least one item".to_string());
    }
    Ok(())
}

fn validate_identifiers(field: &str, values: &[String]) -> Result<(), String> {
    for value in values {
        validate_identifier(field, value)?;
    }
    Ok(())
}

fn validate_identifier(field: &str, value: &str) -> Result<(), String> {
    let mut bytes = value.bytes();
    if !matches!(bytes.next(), Some(b'a'..=b'z')) {
        return Err(format!("invalid {field} `{value}`"));
    }
    if bytes.all(|byte| matches!(byte, b'a'..=b'z' | b'0'..=b'9' | b'_' | b'-')) {
        Ok(())
    } else {
        Err(format!("invalid {field} `{value}`"))
    }
}

fn validate_optional_non_empty(field: &str, value: Option<&str>) -> Result<(), String> {
    if matches!(value, Some("")) {
        Err(format!("{field} must not be empty"))
    } else {
        Ok(())
    }
}

fn validate_non_empty_values(field: &str, values: &[String]) -> Result<(), String> {
    for value in values {
        if value.is_empty() {
            return Err(format!("{field} must not be empty"));
        }
    }
    Ok(())
}

fn validate_unique_values(field: &str, values: &[String]) -> Result<(), String> {
    let mut seen = HashSet::new();
    for value in values {
        if !seen.insert(value.as_str()) {
            return Err(format!("duplicate {field} `{value}`"));
        }
    }
    Ok(())
}

fn validate_optional_event(value: Option<&str>) -> Result<(), String> {
    let Some(value) = value else {
        return Ok(());
    };
    match value {
        "pre-tool" | "permission-request" | "post-tool" | "user-prompt" | "session-start"
        | "subagent-start" | "subagent-stop" | "stop" => Ok(()),
        _ => Err(format!("unsupported event `{value}`")),
    }
}

fn validate_optional_platform(value: Option<&str>) -> Result<(), String> {
    let Some(value) = value else {
        return Ok(());
    };
    match value {
        "codex" | "claude" | "unknown" => Ok(()),
        _ => Err(format!("unsupported platform `{value}`")),
    }
}

fn validate_binary_name(field: &str, value: &str) -> Result<(), String> {
    if !value.is_empty()
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'.' | b'-'))
    {
        Ok(())
    } else {
        Err(format!("invalid {field} `{value}`"))
    }
}

impl ConfigFile {
    fn validate_protocol(&self) -> Result<(), String> {
        expect_optional_field(
            "schemaId",
            self.schema_id.as_deref(),
            CLIENT_HOOK_CONFIG_SCHEMA_ID,
        )?;
        expect_optional_field(
            "schemaVersion",
            self.schema_version.as_deref(),
            CLIENT_HOOK_CONFIG_SCHEMA_VERSION,
        )?;
        expect_optional_field("protocolId", self.protocol_id.as_deref(), HOOK_PROTOCOL_ID)?;
        expect_optional_field(
            "protocolVersion",
            self.protocol_version.as_deref(),
            HOOK_PROTOCOL_VERSION,
        )?;
        Ok(())
    }
}

fn expect_optional_field(field: &str, actual: Option<&str>, expected: &str) -> Result<(), String> {
    if actual.is_some_and(|actual| actual != expected) {
        return Err(format!("expected {field}={expected}"));
    }
    Ok(())
}

impl TryFrom<RuleConfig> for CompiledHookRule {
    type Error = String;

    fn try_from(config: RuleConfig) -> Result<Self, Self::Error> {
        Ok(Self {
            id: config.id,
            priority: config.priority,
            decision: config.decision,
            reason_kind: config
                .reason_kind
                .as_deref()
                .map(parse_reason_kind)
                .transpose()?
                .unwrap_or(ReasonKind::None),
            message: config.message,
            language_ids: config.language_ids,
            event: config.event,
            platform: config.platform,
            match_config: RuleMatch::try_from(config.match_config)?,
            routes: config
                .routes
                .into_iter()
                .map(RuleRoute::try_from)
                .collect::<Result<Vec<_>, _>>()?,
        })
    }
}

impl TryFrom<RuleMatchConfig> for RuleMatch {
    type Error = String;

    fn try_from(config: RuleMatchConfig) -> Result<Self, Self::Error> {
        let mut tool_any = config.tool_any;
        if let Some(tool) = config.tool {
            tool_any.push(tool);
        }
        Ok(Self {
            tool_any,
            command_any: config.command_any,
            command_contains_any: config.command_contains_any,
            path_any: config.path_any,
            path_glob_any: compile_globs(config.path_glob_any)?,
        })
    }
}

impl TryFrom<RuleRouteConfig> for RuleRoute {
    type Error = String;

    fn try_from(config: RuleRouteConfig) -> Result<Self, Self::Error> {
        Ok(Self {
            provider_id: config.provider_id,
            language_id: config.language_id,
            binary: config.binary,
            kind: parse_route_kind(&config.kind)?,
            argv: config.argv,
            stdin_mode: config.stdin_mode,
        })
    }
}

fn compile_globs(patterns: Vec<String>) -> Result<Option<GlobSet>, String> {
    if patterns.is_empty() {
        return Ok(None);
    }
    let mut builder = GlobSetBuilder::new();
    for pattern in patterns {
        let glob = GlobBuilder::new(&pattern)
            .literal_separator(true)
            .build()
            .map_err(|error| format!("invalid pathGlobAny pattern `{pattern}`: {error}"))?;
        builder.add(glob);
    }
    builder
        .build()
        .map(Some)
        .map_err(|error| format!("failed to compile pathGlobAny patterns: {error}"))
}

fn parse_reason_kind(value: &str) -> Result<ReasonKind, String> {
    match value {
        "none" => Ok(ReasonKind::None),
        "direct-source-read" => Ok(ReasonKind::DirectSourceRead),
        "bulk-source-dump" => Ok(ReasonKind::BulkSourceDump),
        "raw-broad-search" => Ok(ReasonKind::RawBroadSearch),
        "agent-search-json" => Ok(ReasonKind::AgentSearchJson),
        "subagent-receipt-required" => Ok(ReasonKind::SubagentReceiptRequired),
        _ => Err(format!("unsupported reasonKind `{value}`")),
    }
}

fn parse_route_kind(value: &str) -> Result<DecisionRouteKind, String> {
    match value {
        "prime" => Ok(DecisionRouteKind::Prime),
        "owner" => Ok(DecisionRouteKind::Owner),
        "query" => Ok(DecisionRouteKind::Query),
        "fzf" => Ok(DecisionRouteKind::Fzf),
        "read" => Ok(DecisionRouteKind::Read),
        "deps" => Ok(DecisionRouteKind::Deps),
        "api" => Ok(DecisionRouteKind::Api),
        "ingest" => Ok(DecisionRouteKind::Ingest),
        "tests" => Ok(DecisionRouteKind::Tests),
        "check-changed" => Ok(DecisionRouteKind::CheckChanged),
        _ => Err(format!("unsupported route kind `{value}`")),
    }
}

fn canonical_event(value: &str) -> String {
    value.to_ascii_lowercase().replace('_', "-")
}

fn default_enabled() -> bool {
    true
}
