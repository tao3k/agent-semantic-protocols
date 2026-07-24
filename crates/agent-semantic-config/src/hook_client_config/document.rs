//! Parses global `asp` hook client configuration from TOML.

use figment::{
    Figment,
    providers::{Format, Toml},
};
use serde::Deserialize;
use std::{
    collections::{BTreeMap, HashSet},
    path::Path,
};

use super::validation::validate_config;

use super::agent_runtime::HookClientAgentsConfig;
use super::routing::HookClientRuleConfig;

/// Schema id for hook client config.
pub const CLIENT_HOOK_CONFIG_SCHEMA_ID: &str = "agent.semantic-protocols.hook.client-config";
/// Schema version for hook client config.
pub const CLIENT_HOOK_CONFIG_SCHEMA_VERSION: &str = "1";

pub(super) const HOOK_PROTOCOL_ID: &str = "agent.semantic-protocols.hook";
pub(super) const HOOK_PROTOCOL_VERSION: &str = "1";

const DEFAULT_HOOK_CLIENT_CONFIG_TEMPLATE: &str = include_str!("../../templates/hooks/config.toml");

/// Parsed and validated project-local hook client config.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct HookClientConfigFile {
    #[serde(default, rename = "wrapper_match")]
    pub wrapper_match: WrapperMatchMode,
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
pub struct AspProjectHookConfig {
    #[serde(default)]
    pub agents: HookClientAgentsConfig,
    #[serde(default)]
    pub rules: Vec<HookClientRuleConfig>,
}

/// Merge project hook declarations over the managed v1 config by stable identity.
///
/// Rules replace complete rules with the same `id`; resident agents replace complete
/// identities with the same `name`. Field-level merging is intentionally unsupported,
/// so every declaration remains one auditable policy unit.
pub fn merge_asp_project_hook_config(
    mut base: HookClientConfigFile,
    project: AspProjectConfigFile,
) -> Result<HookClientConfigFile, String> {
    let mut resident_names = HashSet::new();
    for resident in &project.hook.agents.resident_agents {
        if !resident_names.insert(resident.name.as_str()) {
            return Err(format!(
                "project hook declares resident agent `{}` more than once",
                resident.name
            ));
        }
    }
    let mut rule_ids = HashSet::new();
    for rule in &project.hook.rules {
        if !rule_ids.insert(rule.id.as_str()) {
            return Err(format!(
                "project hook declares rule `{}` more than once",
                rule.id
            ));
        }
    }

    for resident in project.hook.agents.resident_agents {
        if let Some(index) = base
            .agents
            .resident_agents
            .iter()
            .position(|existing| existing.name == resident.name)
        {
            base.agents.resident_agents[index] = resident;
        } else {
            base.agents.resident_agents.push(resident);
        }
    }
    for rule in project.hook.rules {
        if let Some(index) = base
            .rules
            .iter()
            .position(|existing| existing.id == rule.id)
        {
            base.rules[index] = rule;
        } else {
            base.rules.push(rule);
        }
    }
    validate_config(&base)?;
    Ok(base)
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

/// Render the seed global hook config file.
pub fn default_hook_client_config_template() -> String {
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

/// Render a hook client message template with `{{key}}` placeholders.
pub fn render_hook_client_message_template(template: &str, values: &[(&str, &str)]) -> String {
    let mut rendered = template.to_string();
    for (key, value) in values {
        rendered = rendered.replace(&format!("{{{{{key}}}}}"), value);
    }
    rendered.trim().to_string()
}

/// Load, parse, and validate project-local hook config.
pub fn load_hook_client_config_file(path: &Path) -> Result<HookClientConfigFile, String> {
    if !path.is_file() {
        return Err(format!(
            "hook client config does not exist: {}",
            path.display()
        ));
    }
    let parsed = Figment::from(Toml::file(path))
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

pub(super) fn default_enabled() -> bool {
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
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum WrapperMatchMode {
    #[default]
    Enable,
}
