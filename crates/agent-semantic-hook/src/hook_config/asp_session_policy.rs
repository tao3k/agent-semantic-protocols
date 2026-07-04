use agent_semantic_config::{HookClientAgentsConfig, HookClientResidentAgentConfig};

#[derive(Debug)]
/// Compiled ASP command routing policy for root and child agent sessions.
pub struct AspSessionPolicy {
    enabled: bool,
    resident_child_name: String,
    resident_codex_agent_name: String,
    main_allowed_asp_command_prefixes: Vec<Vec<String>>,
    testing_enabled: bool,
    testing_resident_child_name: String,
    testing_resident_codex_agent_name: String,
    testing_command_prefixes: Vec<Vec<String>>,
}

impl Default for AspSessionPolicy {
    fn default() -> Self {
        Self {
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
            testing_enabled: true,
            testing_resident_child_name: "asp-testing".to_string(),
            testing_resident_codex_agent_name: "asp_testing".to_string(),
            testing_command_prefixes: [
                vec!["cargo".to_string(), "test".to_string()],
                vec!["cargo".to_string(), "check".to_string()],
                vec!["cargo".to_string(), "build".to_string()],
                vec!["pytest".to_string()],
                vec!["uv".to_string(), "run".to_string(), "pytest".to_string()],
                vec!["just".to_string(), "test".to_string()],
            ]
            .into_iter()
            .collect(),
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

    /// Return whether an `asp` command at `asp_index` is allowed in the root session.
    pub fn main_asp_command_allowed(&self, tokens: &[String], asp_index: usize) -> bool {
        self.main_allowed_asp_command_prefixes
            .iter()
            .any(|prefix| command_prefix_matches(tokens, asp_index, prefix))
    }

    /// Return the resident child session name used for ASP testing.
    pub fn testing_resident_child_name(&self) -> &str {
        &self.testing_resident_child_name
    }

    /// Return the configured Codex agent role used for ASP testing.
    pub fn testing_resident_codex_agent_name(&self) -> &str {
        &self.testing_resident_codex_agent_name
    }

    /// Return whether a command must be routed to the testing child session.
    pub fn testing_command_matches(&self, tokens: &[String]) -> bool {
        self.testing_enabled
            && self
                .testing_command_prefixes
                .iter()
                .any(|prefix| command_prefix_matches_wrapped(tokens, prefix))
    }
}

impl TryFrom<HookClientAgentsConfig> for AspSessionPolicy {
    type Error = String;

    fn try_from(config: HookClientAgentsConfig) -> Result<Self, Self::Error> {
        let asp_explore = find_resident_agent(&config, "asp-explore", "asp-explore")?.clone();
        let asp_testing = find_resident_agent(&config, "asp-testing", "asp-testing")?.clone();
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
        let testing_command_prefixes = asp_testing
            .command_prefixes
            .iter()
            .map(|prefix| {
                command_prefix_tokens(
                    "agents.residentAgents[asp-testing].commandPrefixes[]",
                    prefix,
                )
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Self {
            enabled: asp_explore.enabled,
            resident_child_name: asp_explore.name,
            resident_codex_agent_name: asp_explore.codex_agent_name,
            main_allowed_asp_command_prefixes,
            testing_enabled: asp_testing.enabled,
            testing_resident_child_name: asp_testing.name,
            testing_resident_codex_agent_name: asp_testing.codex_agent_name,
            testing_command_prefixes,
        })
    }
}

fn find_resident_agent<'a>(
    config: &'a HookClientAgentsConfig,
    name: &str,
    role: &str,
) -> Result<&'a HookClientResidentAgentConfig, String> {
    config
        .resident_agents
        .iter()
        .find(|agent| agent.name == name || agent.role == role)
        .ok_or_else(|| format!("agents.residentAgents must include {name}"))
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

fn command_prefix_matches_at(tokens: &[String], start_index: usize, prefix: &[String]) -> bool {
    tokens
        .iter()
        .skip(start_index)
        .zip(prefix.iter())
        .all(|(token, expected)| token.eq_ignore_ascii_case(expected))
        && tokens.len() >= start_index + prefix.len()
}

fn command_prefix_matches_wrapped(tokens: &[String], prefix: &[String]) -> bool {
    command_prefix_matches_at(tokens, command_start_after_wrappers(tokens), prefix)
}

fn command_start_after_wrappers(tokens: &[String]) -> usize {
    let mut index = 0;
    if tokens
        .get(index)
        .is_some_and(|token| token.eq_ignore_ascii_case("direnv"))
        && tokens
            .get(index + 1)
            .is_some_and(|token| token.eq_ignore_ascii_case("exec"))
    {
        index += 2;
        if index < tokens.len() {
            index += 1;
        }
    }
    if tokens
        .get(index)
        .is_some_and(|token| is_env_command_token(token))
    {
        index += 1;
        while tokens
            .get(index)
            .is_some_and(|token| is_env_assignment_token(token))
        {
            index += 1;
        }
    }
    index
}

fn is_env_command_token(token: &str) -> bool {
    token.rsplit('/').next() == Some("env")
}

fn is_env_assignment_token(token: &str) -> bool {
    let Some((name, _value)) = token.split_once('=') else {
        return false;
    };
    !name.is_empty()
        && name
            .chars()
            .all(|character| character == '_' || character.is_ascii_alphanumeric())
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
