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
    pub contract_fingerprint: Option<String>,
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
    pub execution_lanes: HookClientExecutionLanesConfig,
    #[serde(default)]
    pub asp_command_intent_policy: HookClientAspCommandIntentPolicyConfig,
    #[serde(default)]
    pub rules: Vec<HookClientRuleConfig>,
}

/// Parser-owned policy for classifying public `asp <language>` commands.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct HookClientAspCommandIntentPolicyConfig {
    #[serde(default)]
    pub control_plane: HookClientAspControlPlaneIntentConfig,
    #[serde(default)]
    pub reasoning: HookClientAspReasoningIntentConfig,
    #[serde(default)]
    pub exact_evidence: HookClientAspExactEvidenceIntentConfig,
    #[serde(default)]
    pub direct_read_fallback: HookClientAspDirectReadFallbackIntentConfig,
    #[serde(default)]
    pub invalid_evidence: HookClientAspInvalidEvidenceIntentConfig,
}

impl Default for HookClientAspCommandIntentPolicyConfig {
    fn default() -> Self {
        Self {
            control_plane: HookClientAspControlPlaneIntentConfig::default(),
            reasoning: HookClientAspReasoningIntentConfig::default(),
            exact_evidence: HookClientAspExactEvidenceIntentConfig::default(),
            direct_read_fallback: HookClientAspDirectReadFallbackIntentConfig::default(),
            invalid_evidence: HookClientAspInvalidEvidenceIntentConfig::default(),
        }
    }
}

/// Root ASP commands that are operational control surfaces, not reasoning.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct HookClientAspControlPlaneIntentConfig {
    #[serde(default = "default_asp_control_plane_root_commands")]
    pub root_commands: Vec<String>,
}

impl Default for HookClientAspControlPlaneIntentConfig {
    fn default() -> Self {
        Self {
            root_commands: default_asp_control_plane_root_commands(),
        }
    }
}

/// Commands and routes that remain in the semantic reasoning lane.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct HookClientAspReasoningIntentConfig {
    #[serde(default = "default_asp_reasoning_root_commands")]
    pub root_commands: Vec<String>,
    #[serde(default = "default_asp_policy_true")]
    pub guide_command: bool,
    #[serde(default = "default_asp_reasoning_search_routes")]
    pub search_routes: Vec<String>,
    #[serde(default = "default_asp_reasoning_query_flags")]
    pub query_flags: Vec<String>,
    #[serde(default = "default_asp_policy_true")]
    pub unprojected_query: bool,
}

impl Default for HookClientAspReasoningIntentConfig {
    fn default() -> Self {
        Self {
            root_commands: default_asp_reasoning_root_commands(),
            guide_command: true,
            search_routes: default_asp_reasoning_search_routes(),
            query_flags: default_asp_reasoning_query_flags(),
            unprojected_query: true,
        }
    }
}

/// Projection and selector requirements for exact evidence reads.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct HookClientAspExactEvidenceIntentConfig {
    #[serde(default = "default_asp_exact_evidence_projection_flags")]
    pub query_projection_flags: Vec<String>,
    #[serde(default = "default_asp_exact_evidence_projection_views")]
    pub query_projection_views: Vec<String>,
    #[serde(default = "default_asp_exact_selector_kinds")]
    pub selector_kinds: Vec<String>,
    #[serde(default = "default_asp_policy_true")]
    pub require_same_language: bool,
}

impl Default for HookClientAspExactEvidenceIntentConfig {
    fn default() -> Self {
        Self {
            query_projection_flags: default_asp_exact_evidence_projection_flags(),
            query_projection_views: default_asp_exact_evidence_projection_views(),
            selector_kinds: default_asp_exact_selector_kinds(),
            require_same_language: true,
        }
    }
}

/// Explicit hook provenance values that authorize a bounded direct-read fallback.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct HookClientAspDirectReadFallbackIntentConfig {
    #[serde(default = "default_asp_direct_read_from_hook_values")]
    pub from_hook_values: Vec<String>,
}

impl Default for HookClientAspDirectReadFallbackIntentConfig {
    fn default() -> Self {
        Self {
            from_hook_values: default_asp_direct_read_from_hook_values(),
        }
    }
}

/// Invalid evidence shapes that must be rejected rather than treated as reasoning.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct HookClientAspInvalidEvidenceIntentConfig {
    #[serde(default = "default_asp_policy_true")]
    pub reject_projected_query_without_exact_selector: bool,
    #[serde(default = "default_asp_policy_true")]
    pub reject_cross_language_selector: bool,
}

impl Default for HookClientAspInvalidEvidenceIntentConfig {
    fn default() -> Self {
        Self {
            reject_projected_query_without_exact_selector: true,
            reject_cross_language_selector: true,
        }
    }
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
    pub binary_gate_with_child: Option<String>,
    #[serde(default)]
    pub binary_gate_without_child: Option<String>,
    #[serde(default)]
    pub binary_gate_invalid_child: Option<String>,
    #[serde(default)]
    pub binary_gate_registry_blocked: Option<String>,
    #[serde(default)]
    /// Deprecated compatibility input. Source-access receipts are rule-owned typed materializers.
    pub source_access_compact: Option<String>,
    #[serde(default)]
    /// Deprecated compatibility input. Repeated deny replay no longer carries prompt templates.
    pub source_access_compact_repeated: Option<String>,
    #[serde(default)]
    /// Deprecated compatibility input. Resident agent role text belongs to its agent profile.
    pub source_access_compact_subagent: Option<String>,
}

/// Hook policy for ASP command routing inside agent sessions.
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct HookClientAgentsConfig {
    #[serde(default = "default_resident_agent_configs")]
    pub resident_agents: Vec<HookClientResidentAgentConfig>,
}

/// Hook-selected command-execution lanes keyed by a user-defined lane name.
#[derive(Clone, Debug, Deserialize)]
pub struct HookClientExecutionLanesConfig {
    #[serde(flatten)]
    pub lanes: BTreeMap<String, HookClientExecutionLaneConfig>,
}

impl Default for HookClientExecutionLanesConfig {
    fn default() -> Self {
        Self {
            lanes: BTreeMap::new(),
        }
    }
}

/// One parser-owned deterministic execution lane.
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct HookClientExecutionLaneConfig {
    #[serde(default = "default_enabled")]
    pub enabled: bool,
    #[serde(default)]
    pub transport: HookClientExecutionTransport,
    #[serde(default = "default_testing_resident_agent_name")]
    pub resident_name: String,
    #[serde(default = "default_asp_testing_command_prefixes")]
    pub command_prefixes: Vec<String>,
    #[serde(default = "default_execution_receipt_kind")]
    pub receipt_kind: String,
}

impl Default for HookClientExecutionLaneConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            transport: HookClientExecutionTransport::ResidentAgent,
            resident_name: default_testing_resident_agent_name(),
            command_prefixes: default_asp_testing_command_prefixes(),
            receipt_kind: default_execution_receipt_kind(),
        }
    }
}

/// Host transport used for one deterministic execution lane.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub enum HookClientExecutionTransport {
    /// Execute the exact matched command in the current root session.
    #[default]
    CurrentSession,
    /// Route the exact matched command to the configured resident agent.
    ResidentAgent,
}

impl HookClientExecutionTransport {
    /// Stable configuration and receipt spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::CurrentSession => "current-session",
            Self::ResidentAgent => "resident-agent",
        }
    }
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
            role: "asp_testing".to_string(),
            roles: vec![
                "subagent".to_string(),
                "testing".to_string(),
                "build".to_string(),
            ],
            permissions: vec!["workspace-write".to_string()],
            codex_agent_name: "asp_testing".to_string(),
            lifecycle: "test-build-command".to_string(),
            session_lifetime: "resident".to_string(),
            main_allowed_asp_command_prefixes: Vec::new(),
            command_prefixes: default_asp_testing_command_prefixes(),
        },
    ]
}

fn default_testing_resident_agent_name() -> String {
    "asp-testing".to_string()
}

fn default_execution_receipt_kind() -> String {
    "asp-testing-execution-v1".to_string()
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
    pub decision_materializer: Option<HookClientDecisionMaterializer>,
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
    /// Exact argument-vector prefixes evaluated at each parsed shell-command stage.
    /// For example, `argvPrefixAny = [["rm", "-rf"]]` matches `rm -rf target`.
    #[serde(default)]
    pub argv_prefix_any: Vec<Vec<String>>,
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

#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum HookClientDecisionMaterializer {
    AgentSearchJson,
    PromptSearchStrategy,
    ApplyPatch,
    SourceAccess,
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

fn default_asp_policy_true() -> bool {
    true
}

fn default_asp_control_plane_root_commands() -> Vec<String> {
    [
        "guide",
        "providers",
        "tools",
        "wrap",
        "cache",
        "cloud",
        "hook",
        "agent",
        "install",
        "sync",
        "paths",
        "healthcheck",
        "source-access",
        "ast-patch",
        "graph",
    ]
    .into_iter()
    .map(str::to_string)
    .collect()
}

fn default_asp_reasoning_search_routes() -> Vec<String> {
    [
        "prime",
        "pipe",
        "owner",
        "lexical",
        "deps",
        "dependency",
        "failure",
        "reasoning",
        "ingest",
        "guide",
    ]
    .into_iter()
    .map(str::to_string)
    .collect()
}

fn default_asp_reasoning_root_commands() -> Vec<String> {
    ["fd", "rg"].into_iter().map(str::to_string).collect()
}

fn default_asp_reasoning_query_flags() -> Vec<String> {
    ["--term"].into_iter().map(str::to_string).collect()
}

fn default_asp_exact_evidence_projection_flags() -> Vec<String> {
    ["--code", "--content", "--names-only"]
        .into_iter()
        .map(str::to_string)
        .collect()
}

fn default_asp_exact_evidence_projection_views() -> Vec<String> {
    ["metadata"].into_iter().map(str::to_string).collect()
}

fn default_asp_exact_selector_kinds() -> Vec<String> {
    ["item"].into_iter().map(str::to_string).collect()
}

fn default_asp_direct_read_from_hook_values() -> Vec<String> {
    ["direct-source-read"]
        .into_iter()
        .map(str::to_string)
        .collect()
}

/// Parse the embedded default hook config template.
pub fn default_hook_client_config_file() -> Result<HookClientConfigFile, String> {
    toml::from_str(&default_hook_client_config_template())
        .map_err(|error| format!("failed to parse default hook client config template: {error}"))
}

const HOOK_CLIENT_CONFIG_SCHEMA: &str =
    include_str!("../../../../schemas/semantic-agent-hook-client-config.v1.schema.json");

/// Stable identity for the parser-visible hook config contract embedded in ASP.
pub fn hook_client_contract_fingerprint() -> String {
    let mut hash = 0xcbf29ce484222325_u64;
    for component in [
        CLIENT_HOOK_CONFIG_SCHEMA_ID,
        CLIENT_HOOK_CONFIG_SCHEMA_VERSION,
        HOOK_PROTOCOL_ID,
        HOOK_PROTOCOL_VERSION,
        env!("CARGO_PKG_VERSION"),
        HOOK_CLIENT_CONFIG_SCHEMA,
        DEFAULT_HOOK_CLIENT_CONFIG_TEMPLATE,
    ] {
        for byte in component.as_bytes().iter().copied().chain([0]) {
            hash ^= u64::from(byte);
            hash = hash.wrapping_mul(0x100000001b3);
        }
    }
    format!("hook-client-v1-{hash:016x}")
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
        .replace(
            "@HOOK_CLIENT_CONTRACT_FINGERPRINT@",
            &hook_client_contract_fingerprint(),
        )
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
