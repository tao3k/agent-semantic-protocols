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

use super::validation::validate_config;

/// Schema id for hook client config.
pub const CLIENT_HOOK_CONFIG_SCHEMA_ID: &str = "agent.semantic-protocols.hook.client-config";
/// Schema version for hook client config.
pub const CLIENT_HOOK_CONFIG_SCHEMA_VERSION: &str = "1";

pub(super) const HOOK_PROTOCOL_ID: &str = "agent.semantic-protocols.hook";
pub(super) const HOOK_PROTOCOL_VERSION: &str = "1";

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

const DEFAULT_HOOK_CLIENT_CONFIG_TEMPLATE: &str = include_str!("../../templates/hooks/config.toml");

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
    pub(crate) register: Option<String>,
    #[serde(default)]
    pub(crate) list: Option<String>,
    #[serde(default)]
    pub(crate) show: Option<String>,
    #[serde(default)]
    pub(crate) reuse: Option<String>,
    #[serde(default)]
    pub(crate) status: Option<String>,
}

impl HookClientAgentSessionGuideConfig {
    /// Construct a guide config from explicit optional sections.
    #[must_use]
    pub fn new(
        register: Option<String>,
        list: Option<String>,
        show: Option<String>,
        reuse: Option<String>,
        status: Option<String>,
    ) -> Self {
        Self {
            register,
            list,
            show,
            reuse,
            status,
        }
    }

    /// Guide command for registering a child session.
    #[must_use]
    pub fn register(&self) -> Option<&str> {
        self.register.as_deref()
    }

    /// Guide command for listing registered child sessions.
    #[must_use]
    pub fn list(&self) -> Option<&str> {
        self.list.as_deref()
    }

    /// Guide command for showing one child session.
    #[must_use]
    pub fn show(&self) -> Option<&str> {
        self.show.as_deref()
    }

    /// Guide command for reusing a registered child session.
    #[must_use]
    pub fn reuse(&self) -> Option<&str> {
        self.reuse.as_deref()
    }

    /// Guide command for checking child session status.
    #[must_use]
    pub fn status(&self) -> Option<&str> {
        self.status.as_deref()
    }

    /// Mutable guide command for registering a child session.
    #[must_use]
    pub fn register_mut(&mut self) -> Option<&mut String> {
        self.register.as_mut()
    }

    /// Mutable guide command for listing registered child sessions.
    #[must_use]
    pub fn list_mut(&mut self) -> Option<&mut String> {
        self.list.as_mut()
    }

    /// Mutable guide command for showing one child session.
    #[must_use]
    pub fn show_mut(&mut self) -> Option<&mut String> {
        self.show.as_mut()
    }

    /// Mutable guide command for reusing a registered child session.
    #[must_use]
    pub fn reuse_mut(&mut self) -> Option<&mut String> {
        self.reuse.as_mut()
    }

    /// Mutable guide command for checking child session status.
    #[must_use]
    pub fn status_mut(&mut self) -> Option<&mut String> {
        self.status.as_mut()
    }
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

/// One configured resident agent lane for hook routing.
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct HookClientResidentAgentConfig {
    #[serde(default = "default_enabled")]
    pub enabled: bool,
    pub name: String,
    pub role: String,
    #[serde(default)]
    pub roles: Vec<String>,
    #[serde(default)]
    pub permissions: Vec<String>,
    #[serde(default)]
    pub codex_agent_name: String,
    pub lifecycle: String,
    #[serde(default = "default_session_lifetime")]
    pub session_lifetime: String,
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
            roles: default_explore_resident_agent_roles(),
            permissions: default_explore_resident_agent_permissions(),
            codex_agent_name: default_explore_resident_codex_agent_name(),
            lifecycle: "asp-command".to_string(),
            session_lifetime: "resident".to_string(),
            main_allowed_asp_command_prefixes: default_asp_session_policy_main_allowed_prefixes(),
            command_prefixes: Vec::new(),
        },
        HookClientResidentAgentConfig {
            enabled: true,
            name: default_testing_resident_agent_name(),
            role: default_testing_resident_agent_role(),
            roles: default_testing_resident_agent_roles(),
            permissions: default_testing_resident_agent_permissions(),
            codex_agent_name: default_testing_resident_codex_agent_name(),
            lifecycle: "testing-command".to_string(),
            session_lifetime: "resident".to_string(),
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
    pub(crate) enabled: bool,
    #[serde(default = "default_agent_org_artifacts_inactive_after_minutes")]
    pub(crate) inactive_after_minutes: u64,
    pub(crate) artifacts_path: String,
    pub(crate) entry_skill_path: String,
    #[serde(default)]
    pub(crate) archive_warning: HookClientAgentOrgArtifactsArchiveWarningConfig,
}

impl HookClientAgentOrgArtifactsConfig {
    /// Return whether Org artifact workflow checks are enabled.
    #[must_use]
    pub fn enabled(&self) -> bool {
        self.enabled
    }

    /// Inactivity window in minutes before artifact warnings apply.
    #[must_use]
    pub fn inactive_after_minutes(&self) -> u64 {
        self.inactive_after_minutes
    }

    /// Configured artifact root path.
    #[must_use]
    pub fn artifacts_path(&self) -> &str {
        &self.artifacts_path
    }

    /// Configured entry skill path.
    #[must_use]
    pub fn entry_skill_path(&self) -> &str {
        &self.entry_skill_path
    }

    /// Archive warning policy for active Org artifacts.
    #[must_use]
    pub fn archive_warning(&self) -> &HookClientAgentOrgArtifactsArchiveWarningConfig {
        &self.archive_warning
    }
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

fn default_explore_resident_agent_roles() -> Vec<String> {
    ["subagent", "search"]
        .into_iter()
        .map(str::to_string)
        .collect()
}

fn default_explore_resident_agent_permissions() -> Vec<String> {
    ["read-only"].into_iter().map(str::to_string).collect()
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

fn default_testing_resident_agent_roles() -> Vec<String> {
    ["subagent", "testing", "build"]
        .into_iter()
        .map(str::to_string)
        .collect()
}

fn default_testing_resident_agent_permissions() -> Vec<String> {
    ["workspace-write"]
        .into_iter()
        .map(str::to_string)
        .collect()
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

fn default_enabled() -> bool {
    true
}

fn default_session_lifetime() -> String {
    "temporary".to_string()
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
#[path = "../../tests/unit/hook_client_config.rs"]
mod hook_client_config_tests;
