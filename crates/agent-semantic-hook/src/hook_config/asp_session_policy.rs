use agent_semantic_config::HookClientAgentsConfig;

#[derive(Debug)]
/// Compiled ASP command routing policy for root and child agent sessions.
pub struct AspSessionPolicy {
    enabled: bool,
    resident_child_name: String,
    resident_codex_agent_name: String,
}

impl Default for AspSessionPolicy {
    fn default() -> Self {
        Self {
            enabled: true,
            resident_child_name: "asp-explore".to_string(),
            resident_codex_agent_name: "asp_explorer".to_string(),
        }
    }
}

impl AspSessionPolicy {
    /// Return whether ASP session routing policy is enabled.
    pub fn enabled(&self) -> bool {
        self.enabled
    }

    /// Return the resident child session name used for ASP exploration.
    pub fn resident_child_name(&self) -> &str {
        &self.resident_child_name
    }

    /// Return the configured Codex agent role used for ASP exploration.
    pub fn resident_codex_agent_name(&self) -> &str {
        &self.resident_codex_agent_name
    }
}

impl TryFrom<HookClientAgentsConfig> for AspSessionPolicy {
    type Error = String;

    fn try_from(config: HookClientAgentsConfig) -> Result<Self, Self::Error> {
        let asp_explore = config
            .resident_agents
            .iter()
            .find(|agent| agent.name == "asp-explore")
            .ok_or_else(|| "configured asp-explore resident agent is unavailable".to_string())?
            .clone();
        Ok(Self {
            enabled: asp_explore.enabled,
            resident_child_name: asp_explore.name,
            resident_codex_agent_name: asp_explore.codex_agent_name,
        })
    }
}
