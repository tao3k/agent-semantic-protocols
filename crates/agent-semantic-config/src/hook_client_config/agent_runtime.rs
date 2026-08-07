use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

fn default_enabled() -> bool {
    true
}

fn default_session_lifetime() -> String {
    "temporary".to_string()
}

/// Resident agent identities available to declarative hook rule dispatch.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct HookClientAgentsConfig {
    #[serde(default)]
    pub placeholders: BTreeMap<String, String>,
    #[serde(default)]
    pub resident_agents: Vec<HookClientResidentAgentConfig>,
}

/// One resident identity and its runtime permissions/profile projection.
#[derive(Clone, Debug, Serialize, Deserialize)]
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
    #[serde(default = "default_session_lifetime")]
    pub session_lifetime: String,
    #[serde(default)]
    pub focus_mode: HookClientAgentFocusMode,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum HookClientAgentFocusMode {
    #[default]
    Standard,
    Leaf,
}
