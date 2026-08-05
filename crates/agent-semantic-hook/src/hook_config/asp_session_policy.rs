use agent_semantic_config::HookClientAgentsConfig;

#[derive(Debug)]
/// Compiled ASP command routing policy for root and child agent sessions.
pub struct AspSessionPolicy {
    enabled: bool,
    resident_child_name: String,
    resident_codex_agent_name: String,
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
        let resident_name = config
            .placeholders
            .get("explore")
            .ok_or_else(|| "configured explore resident route is unavailable".to_string())?;
        let resident = config
            .resident_agents
            .iter()
            .find(|agent| agent.name == *resident_name)
            .ok_or_else(|| {
                format!("configured explore resident agent `{resident_name}` is unavailable")
            })?
            .clone();
        Ok(Self {
            enabled: resident.enabled,
            resident_child_name: resident.name,
            resident_codex_agent_name: resident.codex_agent_name,
        })
    }
}
