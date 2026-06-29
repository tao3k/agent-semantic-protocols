use agent_semantic_config::HookClientAspSessionPolicyConfig;

#[derive(Debug)]
/// Compiled ASP command routing policy for root and child agent sessions.
pub struct AspSessionPolicy {
    enabled: bool,
    resident_child_name: String,
    main_allowed_asp_command_prefixes: Vec<Vec<String>>,
}

impl Default for AspSessionPolicy {
    fn default() -> Self {
        Self {
            enabled: true,
            resident_child_name: "asp-explore".to_string(),
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
    /// Return whether ASP session routing policy is enabled.
    pub fn enabled(&self) -> bool {
        self.enabled
    }

    /// Return the resident child session name used for ASP exploration.
    pub fn resident_child_name(&self) -> &str {
        &self.resident_child_name
    }

    /// Return whether an `asp` command at `asp_index` is allowed in the root session.
    pub fn main_asp_command_allowed(&self, tokens: &[String], asp_index: usize) -> bool {
        self.main_allowed_asp_command_prefixes
            .iter()
            .any(|prefix| command_prefix_matches(tokens, asp_index, prefix))
    }
}

impl TryFrom<HookClientAspSessionPolicyConfig> for AspSessionPolicy {
    type Error = String;

    fn try_from(config: HookClientAspSessionPolicyConfig) -> Result<Self, Self::Error> {
        let main_allowed_asp_command_prefixes = config
            .main_allowed_asp_command_prefixes
            .iter()
            .map(|prefix| command_prefix_tokens(prefix))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Self {
            enabled: config.enabled,
            resident_child_name: config.resident_child_name,
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

fn command_prefix_tokens(prefix: &str) -> Result<Vec<String>, String> {
    let tokens = prefix
        .split_whitespace()
        .map(|token| token.to_ascii_lowercase())
        .collect::<Vec<_>>();
    if tokens.is_empty() {
        Err("aspSessionPolicy.mainAllowedAspCommandPrefixes[] must not be empty".to_string())
    } else {
        Ok(tokens)
    }
}
