use std::collections::BTreeMap;

use serde::Deserialize;

fn default_enabled() -> bool {
    true
}

fn default_session_lifetime() -> String {
    "temporary".to_string()
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

pub(super) fn default_testing_resident_agent_name() -> String {
    "asp-testing".to_string()
}

fn default_execution_receipt_kind() -> String {
    "asp-testing-execution-v1".to_string()
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

pub(super) fn default_asp_testing_command_prefixes() -> Vec<String> {
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
