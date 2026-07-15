use agent_semantic_config::HookClientAgentsConfig;

#[derive(Debug)]
/// Compiled ASP command routing policy for root and child agent sessions.
pub struct AspSessionPolicy {
    command_intent_policy: agent_semantic_config::HookClientAspCommandIntentPolicyConfig,
    enabled: bool,
    resident_child_name: String,
    resident_codex_agent_name: String,
    main_allowed_asp_command_prefixes: Vec<Vec<String>>,
}

impl Default for AspSessionPolicy {
    fn default() -> Self {
        Self {
            command_intent_policy:
                agent_semantic_config::HookClientAspCommandIntentPolicyConfig::default(),
            enabled: true,
            resident_child_name: "asp-explore".to_string(),
            resident_codex_agent_name: "asp_explorer".to_string(),
            main_allowed_asp_command_prefixes: [
                vec!["help".to_string()],
                vec!["--help".to_string()],
                vec!["-h".to_string()],
                vec!["agent".to_string(), "session".to_string()],
                vec!["org".to_string(), "recall".to_string()],
                vec!["org".to_string(), "capture".to_string()],
            ]
            .into_iter()
            .collect(),
        }
    }
}

impl AspSessionPolicy {
    pub(super) fn with_command_intent_policy(
        mut self,
        policy: agent_semantic_config::HookClientAspCommandIntentPolicyConfig,
    ) -> Self {
        self.command_intent_policy = policy;
        self
    }

    /// Parser-owned command taxonomy shared by hook and session routing.
    pub fn command_intent_policy(
        &self,
    ) -> &agent_semantic_config::HookClientAspCommandIntentPolicyConfig {
        &self.command_intent_policy
    }

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

    /// Return whether an `asp` command at `asp_index` is allowed in the root session.
    pub fn main_asp_command_allowed(&self, tokens: &[String], asp_index: usize) -> bool {
        self.main_allowed_asp_command_prefixes
            .iter()
            .any(|prefix| command_prefix_matches(tokens, asp_index, prefix))
    }
}

impl TryFrom<HookClientAgentsConfig> for AspSessionPolicy {
    type Error = String;

    fn try_from(config: HookClientAgentsConfig) -> Result<Self, Self::Error> {
        let default_config = HookClientAgentsConfig::default();
        let asp_explore = config
            .resident_agents
            .iter()
            .find(|agent| agent.lifecycle == "asp-command")
            .or_else(|| {
                default_config
                    .resident_agents
                    .iter()
                    .find(|agent| agent.lifecycle == "asp-command")
            })
            .ok_or_else(|| "built-in asp-command resident agent is unavailable".to_string())?
            .clone();
        let main_allowed_asp_command_prefixes = asp_explore
            .main_allowed_asp_command_prefixes
            .iter()
            .map(|prefix| {
                command_prefix_tokens(
                    "agents.residentAgents[asp-explore].mainAllowedAspCommandPrefixes[]",
                    prefix,
                )
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Self {
            command_intent_policy:
                agent_semantic_config::HookClientAspCommandIntentPolicyConfig::default(),
            enabled: asp_explore.enabled,
            resident_child_name: asp_explore.name,
            resident_codex_agent_name: asp_explore.codex_agent_name,
            main_allowed_asp_command_prefixes,
        })
    }
}

fn command_prefix_matches(tokens: &[String], asp_index: usize, prefix: &[String]) -> bool {
    let command_start = asp_index + 1;
    if tokens.len() <= command_start {
        return prefix.len() == 1 && prefix[0] == "help";
    }
    tokens
        .iter()
        .skip(command_start)
        .zip(prefix.iter())
        .all(|(token, expected)| token.eq_ignore_ascii_case(expected))
        && tokens.len() >= command_start + prefix.len()
}

fn command_prefix_tokens(field: &str, prefix: &str) -> Result<Vec<String>, String> {
    let tokens = prefix
        .split_whitespace()
        .map(|token| token.to_ascii_lowercase())
        .collect::<Vec<_>>();
    if tokens.is_empty() {
        Err(format!("{field} must not be empty"))
    } else {
        Ok(tokens)
    }
}
