//! Parses global `asp` hook client configuration from TOML.

use figment::{
    Figment,
    providers::{Format, Serialized, Toml},
};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, HashSet},
    path::Path,
};

/// Schema id for hook client config.
pub const CLIENT_HOOK_CONFIG_SCHEMA_ID: &str = "agent.semantic-protocols.hook.client-config";
/// Schema version for hook client config.
pub const CLIENT_HOOK_CONFIG_SCHEMA_VERSION: &str = "1";

const HOOK_PROTOCOL_ID: &str = "agent.semantic-protocols.hook";
const HOOK_PROTOCOL_VERSION: &str = "1";

const DEFAULT_HOOK_CLIENT_SOURCE_EXTENSIONS: &[&str] = &[
    ".rs",
    ".py",
    ".ts",
    ".tsx",
    ".js",
    ".jsx",
    ".mts",
    ".cts",
    ".mjs",
    ".cjs",
    ".ss",
    ".ssi",
    ".scm",
    ".sld",
    ".jl",
    ".org",
    ".org_archive",
    ".md",
    ".markdown",
];

const DEFAULT_HOOK_CLIENT_CONFIG_TEMPLATE: &str = include_str!("../templates/hooks/config.toml");

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
    pub agent_org_artifacts: Option<HookClientAgentOrgArtifactsConfig>,
    #[serde(default)]
    pub recovery_prompt: HookClientRecoveryPromptConfig,
    #[serde(default)]
    pub agent_session_guide: HookClientAgentSessionGuideConfig,
    #[serde(default)]
    pub agent_session_messages: HookClientAgentSessionMessagesConfig,
    #[serde(default)]
    pub agents: HookClientAgentsConfig,
    #[serde(default)]
    pub rules: Vec<HookClientRuleConfig>,
}

/// Optional hook recovery prompt template and per-client agent-flow fragments.
#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct HookClientRecoveryPromptConfig {
    #[serde(default)]
    pub template: Option<String>,
    #[serde(default)]
    pub codex_agent_flow: Option<String>,
    #[serde(default)]
    pub claude_agent_flow: Option<String>,
    #[serde(default)]
    pub default_agent_flow: Option<String>,
}

/// Optional agent-facing guide text for session registry recovery.
#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct HookClientAgentSessionGuideConfig {
    #[serde(default)]
    pub register: Option<String>,
    #[serde(default)]
    pub list: Option<String>,
    #[serde(default)]
    pub show: Option<String>,
    #[serde(default)]
    pub reuse: Option<String>,
    #[serde(default)]
    pub status: Option<String>,
}

/// Optional agent-facing hook decision text for session routing.
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct HookClientAgentSessionMessagesConfig {
    #[serde(default)]
    pub session_start_reuse: Option<String>,
    #[serde(default)]
    pub session_start_bootstrap: Option<String>,
    #[serde(default)]
    pub missing_resident_explore: Option<String>,
    #[serde(default)]
    pub main_restricted_with_child: Option<String>,
    #[serde(default)]
    pub main_restricted_without_child: Option<String>,
    #[serde(default)]
    pub testing_with_child: Option<String>,
    #[serde(default)]
    pub testing_without_child: Option<String>,
    #[serde(default)]
    pub binary_gate_with_child: Option<String>,
    #[serde(default)]
    pub binary_gate_without_child: Option<String>,
    #[serde(default)]
    pub binary_gate_invalid_child: Option<String>,
    #[serde(default)]
    pub binary_gate_registry_blocked: Option<String>,
    #[serde(default)]
    pub source_access_compact: Option<String>,
    #[serde(default)]
    pub source_access_compact_repeated: Option<String>,
    #[serde(default)]
    pub source_access_compact_subagent: Option<String>,
}

/// Hook policy for ASP command routing inside agent sessions.
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct HookClientAgentsConfig {
    #[serde(default = "default_resident_agent_configs")]
    pub resident_agents: Vec<HookClientResidentAgentConfig>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct HookClientResidentAgentConfig {
    #[serde(default = "default_enabled")]
    pub enabled: bool,
    pub name: String,
    pub role: String,
    #[serde(default)]
    pub codex_agent_name: String,
    pub lifecycle: String,
    #[serde(default)]
    pub main_allowed_asp_command_prefixes: Vec<String>,
    #[serde(default)]
    pub command_prefixes: Vec<String>,
}

impl Default for HookClientAgentsConfig {
    fn default() -> Self {
        Self {
            resident_agents: default_resident_agent_configs(),
        }
    }
}

fn default_resident_agent_configs() -> Vec<HookClientResidentAgentConfig> {
    vec![
        HookClientResidentAgentConfig {
            enabled: true,
            name: default_explore_resident_agent_name(),
            role: default_explore_resident_agent_role(),
            codex_agent_name: default_explore_resident_codex_agent_name(),
            lifecycle: "asp-command".to_string(),
            main_allowed_asp_command_prefixes: default_asp_session_policy_main_allowed_prefixes(),
            command_prefixes: Vec::new(),
        },
        HookClientResidentAgentConfig {
            enabled: true,
            name: default_testing_resident_agent_name(),
            role: default_testing_resident_agent_role(),
            codex_agent_name: default_testing_resident_codex_agent_name(),
            lifecycle: "testing-command".to_string(),
            main_allowed_asp_command_prefixes: Vec::new(),
            command_prefixes: default_asp_testing_command_prefixes(),
        },
    ]
}

/// Parsed ASP project config from `.agents/asp.toml`.
#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AspProjectConfigFile {
    #[serde(default)]
    pub hook: AspProjectHookConfig,
}

/// Hook-owned ASP project config.
#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AspProjectHookConfig {}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct HookClientConfigMetadataDefaults {
    schema_id: &'static str,
    schema_version: &'static str,
    protocol_id: &'static str,
    protocol_version: &'static str,
}

/// Agent-facing Org artifact workflow guard from project-local hook config.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct HookClientAgentOrgArtifactsConfig {
    #[serde(default = "default_enabled")]
    pub enabled: bool,
    #[serde(default = "default_agent_org_artifacts_inactive_after_minutes")]
    pub inactive_after_minutes: u64,
    pub artifacts_path: String,
    pub entry_skill_path: String,
    #[serde(default)]
    pub archive_warning: HookClientAgentOrgArtifactsArchiveWarningConfig,
}

/// Warning policy for active Org artifacts that should be archived.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct HookClientAgentOrgArtifactsArchiveWarningConfig {
    #[serde(default = "default_enabled")]
    pub enabled: bool,
    #[serde(default = "default_agent_org_artifacts_archive_warning_threshold")]
    pub active_org_file_threshold: usize,
    #[serde(default = "default_agent_org_artifacts_archives_dir")]
    pub archives_dir: String,
    #[serde(default = "default_agent_org_artifacts_archive_warning_max_reported_files")]
    pub max_reported_files: usize,
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
    Lexical,
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

/// Render the seed global hook config file.
pub fn default_hook_client_config_template() -> String {
    default_hook_client_config_template_for_source_extensions(DEFAULT_HOOK_CLIENT_SOURCE_EXTENSIONS)
}

/// Parse the embedded default hook config template.
pub fn default_hook_client_config_file() -> Result<HookClientConfigFile, String> {
    toml::from_str(&default_hook_client_config_template())
        .map_err(|error| format!("failed to parse default hook client config template: {error}"))
}

/// Render the seed global hook config file for active provider source extensions.
pub fn default_hook_client_config_template_for_source_extensions<I, S>(
    source_extensions: I,
) -> String
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let argv_source_globs = render_argv_source_globs(source_extensions);
    DEFAULT_HOOK_CLIENT_CONFIG_TEMPLATE
        .replace(
            "@CLIENT_HOOK_CONFIG_SCHEMA_ID@",
            CLIENT_HOOK_CONFIG_SCHEMA_ID,
        )
        .replace(
            "@CLIENT_HOOK_CONFIG_SCHEMA_VERSION@",
            CLIENT_HOOK_CONFIG_SCHEMA_VERSION,
        )
        .replace("@HOOK_PROTOCOL_ID@", HOOK_PROTOCOL_ID)
        .replace("@HOOK_PROTOCOL_VERSION@", HOOK_PROTOCOL_VERSION)
        .replace("@ARGV_SOURCE_GLOBS@", &argv_source_globs)
}

/// Render a hook client message template with `{{key}}` placeholders.
pub fn render_hook_client_message_template(template: &str, values: &[(&str, &str)]) -> String {
    let mut rendered = template.to_string();
    for (key, value) in values {
        rendered = rendered.replace(&format!("{{{{{key}}}}}"), value);
    }
    rendered.trim().to_string()
}

fn render_argv_source_globs<I, S>(source_extensions: I) -> String
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let mut seen = HashSet::new();
    let mut lines = Vec::new();
    for source_extension in source_extensions {
        let Some(extension) = normalize_source_extension(source_extension.as_ref()) else {
            continue;
        };
        if seen.insert(extension.clone()) {
            lines.push(format!("  \"*{extension}\", \"**/*{extension}\","));
        }
    }
    if lines.is_empty() {
        return render_argv_source_globs(DEFAULT_HOOK_CLIENT_SOURCE_EXTENSIONS);
    }
    lines.join("\n")
}

fn normalize_source_extension(source_extension: &str) -> Option<String> {
    let extension = source_extension.trim();
    if extension.is_empty() {
        return None;
    }
    let extension = extension
        .strip_prefix("**/*")
        .or_else(|| extension.strip_prefix('*'))
        .unwrap_or(extension);
    if extension.starts_with('.') {
        Some(extension.to_string())
    } else {
        Some(format!(".{extension}"))
    }
}

fn default_explore_resident_agent_name() -> String {
    "asp-explore".to_string()
}

fn default_explore_resident_agent_role() -> String {
    "asp_explorer".to_string()
}

fn default_explore_resident_codex_agent_name() -> String {
    "asp_explorer".to_string()
}

fn default_asp_session_policy_main_allowed_prefixes() -> Vec<String> {
    [
        "help",
        "--help",
        "-h",
        "agent session",
        "org recall",
        "org capture",
    ]
    .into_iter()
    .map(str::to_string)
    .collect()
}

fn default_testing_resident_agent_name() -> String {
    "asp-testing".to_string()
}

fn default_testing_resident_agent_role() -> String {
    "asp_testing".to_string()
}

fn default_testing_resident_codex_agent_name() -> String {
    "asp_testing".to_string()
}

fn default_asp_testing_command_prefixes() -> Vec<String> {
    [
        "cargo test",
        "cargo check",
        "cargo build",
        "pytest",
        "uv run pytest",
        "just test",
    ]
    .into_iter()
    .map(str::to_string)
    .collect()
}

/// Load, parse, and validate project-local hook config.
pub fn load_hook_client_config_file(path: &Path) -> Result<HookClientConfigFile, String> {
    if !path.is_file() {
        return Ok(HookClientConfigFile::default());
    }
    let parsed = Figment::from(Serialized::defaults(hook_client_config_metadata_defaults()))
        .merge(Toml::file(path))
        .extract::<HookClientConfigFile>()
        .map_err(|error| format!("failed to parse {}: {error}", path.display()))?;
    validate_config(&parsed)?;
    Ok(parsed)
}

/// Load the ASP project config. Unknown project sections are ignored here; each
/// subsystem owns its own parsed subset.
pub fn load_asp_project_config_file(path: &Path) -> Result<AspProjectConfigFile, String> {
    if !path.is_file() {
        return Ok(AspProjectConfigFile::default());
    }
    Figment::from(Toml::file(path))
        .extract::<AspProjectConfigFile>()
        .map_err(|error| format!("failed to parse {}: {error}", path.display()))
}

fn hook_client_config_metadata_defaults() -> HookClientConfigMetadataDefaults {
    HookClientConfigMetadataDefaults {
        schema_id: CLIENT_HOOK_CONFIG_SCHEMA_ID,
        schema_version: CLIENT_HOOK_CONFIG_SCHEMA_VERSION,
        protocol_id: HOOK_PROTOCOL_ID,
        protocol_version: HOOK_PROTOCOL_VERSION,
    }
}

fn validate_config(config: &HookClientConfigFile) -> Result<(), String> {
    validate_protocol(config)?;
    validate_agent_org_artifacts(config.agent_org_artifacts.as_ref())?;
    validate_recovery_prompt(&config.recovery_prompt)?;
    validate_agent_session_guide(&config.agent_session_guide)?;
    validate_resident_agents(&config.agents.resident_agents)?;
    validate_unique_rule_ids(&config.rules)?;
    validate_rule_schema_shape(&config.rules)
}

fn validate_recovery_prompt(config: &HookClientRecoveryPromptConfig) -> Result<(), String> {
    validate_optional_non_empty("recoveryPrompt.template", config.template.as_deref())?;
    validate_optional_non_empty(
        "recoveryPrompt.codexAgentFlow",
        config.codex_agent_flow.as_deref(),
    )?;
    validate_optional_non_empty(
        "recoveryPrompt.claudeAgentFlow",
        config.claude_agent_flow.as_deref(),
    )?;
    validate_optional_non_empty(
        "recoveryPrompt.defaultAgentFlow",
        config.default_agent_flow.as_deref(),
    )
}

fn validate_agent_session_guide(config: &HookClientAgentSessionGuideConfig) -> Result<(), String> {
    validate_optional_non_empty("agentSessionGuide.register", config.register.as_deref())?;
    validate_optional_non_empty("agentSessionGuide.list", config.list.as_deref())?;
    validate_optional_non_empty("agentSessionGuide.show", config.show.as_deref())?;
    validate_optional_non_empty("agentSessionGuide.reuse", config.reuse.as_deref())
}

fn validate_resident_agents(configs: &[HookClientResidentAgentConfig]) -> Result<(), String> {
    for config in configs {
        validate_resident_agent(config)?;
    }
    Ok(())
}

fn validate_resident_agent(config: &HookClientResidentAgentConfig) -> Result<(), String> {
    validate_optional_non_empty("agents.residentAgents[].name", Some(config.name.as_str()))?;
    validate_optional_non_empty("agents.residentAgents[].role", Some(config.role.as_str()))?;
    if !config.codex_agent_name.is_empty() {
        validate_optional_non_empty(
            "agents.residentAgents[].codexAgentName",
            Some(config.codex_agent_name.as_str()),
        )?;
    }
    validate_optional_non_empty(
        "agents.residentAgents[].lifecycle",
        Some(config.lifecycle.as_str()),
    )?;
    for prefix in &config.main_allowed_asp_command_prefixes {
        validate_optional_non_empty(
            "agents.residentAgents[].mainAllowedAspCommandPrefixes[]",
            Some(prefix.as_str()),
        )?;
    }
    for prefix in &config.command_prefixes {
        validate_optional_non_empty(
            "agents.residentAgents[].commandPrefixes[]",
            Some(prefix.as_str()),
        )?;
    }
    Ok(())
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

fn validate_agent_org_artifacts(
    config: Option<&HookClientAgentOrgArtifactsConfig>,
) -> Result<(), String> {
    let Some(config) = config else {
        return Ok(());
    };
    if config.inactive_after_minutes == 0 {
        return Err("agentOrgArtifacts.inactiveAfterMinutes must be greater than 0".to_string());
    }
    validate_non_empty("agentOrgArtifacts.artifactsPath", &config.artifacts_path)?;
    validate_non_empty("agentOrgArtifacts.entrySkillPath", &config.entry_skill_path)?;
    validate_agent_org_artifacts_archive_warning(&config.archive_warning)?;
    Ok(())
}

fn validate_agent_org_artifacts_archive_warning(
    config: &HookClientAgentOrgArtifactsArchiveWarningConfig,
) -> Result<(), String> {
    if config.active_org_file_threshold == 0 {
        return Err(
            "agentOrgArtifacts.archiveWarning.activeOrgFileThreshold must be greater than 0"
                .to_string(),
        );
    }
    if config.max_reported_files == 0 {
        return Err(
            "agentOrgArtifacts.archiveWarning.maxReportedFiles must be greater than 0".to_string(),
        );
    }
    validate_non_empty(
        "agentOrgArtifacts.archiveWarning.archivesDir",
        &config.archives_dir,
    )?;
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

fn validate_non_empty(field: &str, value: &str) -> Result<(), String> {
    if value.is_empty() {
        Err(format!("{field} must not be empty"))
    } else {
        Ok(())
    }
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

fn default_agent_org_artifacts_inactive_after_minutes() -> u64 {
    30
}

fn default_agent_org_artifacts_archive_warning_threshold() -> usize {
    10
}

fn default_agent_org_artifacts_archives_dir() -> String {
    "archives".to_string()
}

fn default_agent_org_artifacts_archive_warning_max_reported_files() -> usize {
    5
}

impl Default for HookClientAgentOrgArtifactsArchiveWarningConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            active_org_file_threshold: default_agent_org_artifacts_archive_warning_threshold(),
            archives_dir: default_agent_org_artifacts_archives_dir(),
            max_reported_files: default_agent_org_artifacts_archive_warning_max_reported_files(),
        }
    }
}

#[cfg(test)]
#[path = "../tests/unit/hook_client_config.rs"]
mod hook_client_config_tests;
