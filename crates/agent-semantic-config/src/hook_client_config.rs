//! Parses project-local `asp` hook client configuration from TOML.

use figment::{
    Figment,
    providers::{Format, Toml},
};
use serde::Deserialize;
use std::{
    collections::{BTreeMap, HashSet},
    path::{Path, PathBuf},
};

/// Schema id for project-local hook client config.
pub const CLIENT_HOOK_CONFIG_SCHEMA_ID: &str = "agent.semantic-protocols.hook.client-config";
/// Schema version for project-local hook client config.
pub const CLIENT_HOOK_CONFIG_SCHEMA_VERSION: &str = "1";

const HOOK_PROTOCOL_ID: &str = "agent.semantic-protocols.hook";
const HOOK_PROTOCOL_VERSION: &str = "1";

/// Parsed and validated project-local hook client config.
#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct HookClientConfigFile {
    #[serde(default)]
    pub schema_id: Option<String>,
    #[serde(default)]
    pub schema_version: Option<String>,
    #[serde(default)]
    pub protocol_id: Option<String>,
    #[serde(default)]
    pub protocol_version: Option<String>,
    #[serde(default)]
    pub experimental: BTreeMap<String, BTreeMap<String, bool>>,
    #[serde(default)]
    pub rules: Vec<HookClientRuleConfig>,
}

/// One declarative hook rule from project-local config.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct HookClientRuleConfig {
    pub id: String,
    #[serde(default = "default_enabled")]
    pub enabled: bool,
    #[serde(default)]
    pub priority: i64,
    pub decision: HookClientConfigDecision,
    #[serde(default)]
    pub reason_kind: Option<HookClientConfigReasonKind>,
    #[serde(default)]
    pub message: Option<String>,
    #[serde(default)]
    pub language_ids: Vec<String>,
    #[serde(default)]
    pub event: Option<String>,
    #[serde(default)]
    pub platform: Option<String>,
    #[serde(default, rename = "match")]
    pub match_config: HookClientRuleMatchConfig,
    #[serde(default)]
    pub routes: Vec<HookClientRuleRouteConfig>,
}

/// Rule match axes from project-local hook config.
#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct HookClientRuleMatchConfig {
    #[serde(default)]
    pub tool: Option<String>,
    #[serde(default)]
    pub tool_any: Vec<String>,
    #[serde(default)]
    pub command_any: Vec<String>,
    #[serde(default)]
    pub command_contains_any: Vec<String>,
    #[serde(default)]
    pub path_any: Vec<String>,
    #[serde(default)]
    pub path_glob_any: Vec<String>,
    #[serde(default)]
    pub argv_source_any: Vec<String>,
    #[serde(default)]
    pub argv_source_glob_any: Vec<String>,
    #[serde(default)]
    pub argv_source_exclude_flag_any: Vec<String>,
}

/// Route suggestion from project-local hook config.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct HookClientRuleRouteConfig {
    pub provider_id: String,
    #[serde(default)]
    pub language_id: Option<String>,
    #[serde(default)]
    pub binary: Option<String>,
    pub kind: HookClientConfigRouteKind,
    pub argv: Vec<String>,
    #[serde(default)]
    pub stdin_mode: Option<HookClientConfigStdinMode>,
}

/// Config-level decision spelling for a rule.
#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum HookClientConfigDecision {
    Block,
    Deny,
}

/// Config-level reason category spelling for a rule.
#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum HookClientConfigReasonKind {
    None,
    DirectSourceRead,
    BulkSourceDump,
    RawBroadSearch,
    AgentSearchJson,
    SubagentReceiptRequired,
}

/// Config-level route kind spelling for a rule route.
#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum HookClientConfigRouteKind {
    Prime,
    Owner,
    Query,
    Fzf,
    Read,
    Deps,
    Api,
    Ingest,
    Tests,
    CheckChanged,
}

/// Config-level stdin handling spelling for a route.
#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum HookClientConfigStdinMode {
    None,
    PipeCandidates,
    PipeDiff,
    Unknown,
}

/// Return the versioned project-local hook config path.
pub fn default_hook_client_config_path(project_root: impl AsRef<Path>) -> PathBuf {
    project_root
        .as_ref()
        .join(".codex")
        .join("agent-semantic-protocol")
        .join("hooks")
        .join("config.toml")
}

/// Render the seed project-local hook config file.
pub fn default_hook_client_config_template() -> String {
    format!(
        r#"# Semantic agent client hook config.
# Loaded by `asp hook` on every client hook invocation.
# Generated rules extend the built-in classifier with configurable shell argv evidence.
schemaId = "{CLIENT_HOOK_CONFIG_SCHEMA_ID}"
schemaVersion = "{CLIENT_HOOK_CONFIG_SCHEMA_VERSION}"
protocolId = "{HOOK_PROTOCOL_ID}"
protocolVersion = "{HOOK_PROTOCOL_VERSION}"

# AST patch routing is experimental and disabled for generated Codex hook configs.
[experimental.semanticAstPatch]
enabled = false

[[rules]]
id = "deny-shell-source-argv"
enabled = true
event = "pre-tool"
priority = 80
decision = "deny"
reasonKind = "bulk-source-dump"
message = "Use the language harness instead of shell argv source reads."

[rules.match]
tool = "Bash"
commandAny = ["sed", "perl", "rg", "wl"]
argvSourceGlobAny = [
  "*.rs", "**/*.rs",
  "*.py", "**/*.py",
  "*.ts", "**/*.ts",
  "*.tsx", "**/*.tsx",
  "*.js", "**/*.js",
  "*.jsx", "**/*.jsx",
  "*.mts", "**/*.mts",
  "*.cts", "**/*.cts",
  "*.mjs", "**/*.mjs",
  "*.cjs", "**/*.cjs",
  "*.ss", "**/*.ss",
  "*.scm", "**/*.scm",
  "*.sld", "**/*.sld",
  "*.jl", "**/*.jl",
]
argvSourceExcludeFlagAny = ["--output", "--output-file", "--out", "-o"]
"#
    )
}

/// Load, parse, and validate project-local hook config.
pub fn load_hook_client_config_file(path: &Path) -> Result<HookClientConfigFile, String> {
    if !path.is_file() {
        return Ok(HookClientConfigFile::default());
    }
    let parsed = Figment::from(Toml::file(path))
        .extract::<HookClientConfigFile>()
        .map_err(|error| format!("failed to parse {}: {error}", path.display()))?;
    validate_config(&parsed)?;
    Ok(parsed)
}

fn validate_config(config: &HookClientConfigFile) -> Result<(), String> {
    validate_protocol(config)?;
    validate_unique_rule_ids(&config.rules)?;
    validate_rule_schema_shape(&config.rules)
}

fn validate_protocol(config: &HookClientConfigFile) -> Result<(), String> {
    expect_optional_field(
        "schemaId",
        config.schema_id.as_deref(),
        CLIENT_HOOK_CONFIG_SCHEMA_ID,
    )?;
    expect_optional_field(
        "schemaVersion",
        config.schema_version.as_deref(),
        CLIENT_HOOK_CONFIG_SCHEMA_VERSION,
    )?;
    expect_optional_field(
        "protocolId",
        config.protocol_id.as_deref(),
        HOOK_PROTOCOL_ID,
    )?;
    expect_optional_field(
        "protocolVersion",
        config.protocol_version.as_deref(),
        HOOK_PROTOCOL_VERSION,
    )?;
    Ok(())
}

fn validate_unique_rule_ids(rules: &[HookClientRuleConfig]) -> Result<(), String> {
    let mut seen = HashSet::new();
    for rule in rules {
        if !seen.insert(rule.id.as_str()) {
            return Err(format!("duplicate client hook rule id `{}`", rule.id));
        }
    }
    Ok(())
}

fn validate_rule_schema_shape(rules: &[HookClientRuleConfig]) -> Result<(), String> {
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

fn validate_match_schema_shape(match_config: &HookClientRuleMatchConfig) -> Result<(), String> {
    validate_optional_non_empty("rules[].match.tool", match_config.tool.as_deref())?;
    validate_non_empty_values("rules[].match.toolAny[]", &match_config.tool_any)?;
    validate_non_empty_values("rules[].match.commandAny[]", &match_config.command_any)?;
    validate_non_empty_values(
        "rules[].match.commandContainsAny[]",
        &match_config.command_contains_any,
    )?;
    validate_non_empty_values("rules[].match.pathAny[]", &match_config.path_any)?;
    validate_non_empty_values("rules[].match.pathGlobAny[]", &match_config.path_glob_any)?;
    validate_non_empty_values(
        "rules[].match.argvSourceAny[]",
        &match_config.argv_source_any,
    )?;
    validate_non_empty_values(
        "rules[].match.argvSourceGlobAny[]",
        &match_config.argv_source_glob_any,
    )?;
    validate_non_empty_values(
        "rules[].match.argvSourceExcludeFlagAny[]",
        &match_config.argv_source_exclude_flag_any,
    )?;
    Ok(())
}

fn validate_route_schema_shape(route: &HookClientRuleRouteConfig) -> Result<(), String> {
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

fn expect_optional_field(field: &str, actual: Option<&str>, expected: &str) -> Result<(), String> {
    if actual.is_some_and(|actual| actual != expected) {
        return Err(format!("expected {field}={expected}"));
    }
    Ok(())
}

fn default_enabled() -> bool {
    true
}

#[cfg(test)]
#[path = "../tests/unit/hook_client_config.rs"]
mod hook_client_config_tests;
