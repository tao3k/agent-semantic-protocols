//! Compiled recovery prompt fragments for hook-deny guidance.

use agent_semantic_config::HookClientRecoveryPromptConfig;

const DEFAULT_RECOVERY_TEMPLATE: &str = r#"ASP denied `{reason}`. Do not retry raw source tools.
Use the ASP route below, or delegate the lookup to the registered `asp-explore` session when available.
Return compact evidence only.
{routes}
{agent_flow}
"#;

const CODEX_AGENT_FLOW: &str = r#"Codex: start the ASP explorer subagent for ASP search/query work. Call `spawn_agent` once with `agent_type="asp_explorer"` and name/purpose `asp-explore`.
When the subagent returns its child session id, register it from this root session: `asp agent session register --name asp-explore --child-session-id <child-session-id> --role asp-explore`. ASP resolves the root and parent session from the active agent environment.
Forward ASP search/query work to `asp-explore`; keep the root agent on session, checkpoint, and recovery commands.
"#;

const CLAUDE_AGENT_FLOW: &str = r#"Claude: run the selected safe route directly in this thread. Use Claude-native helper agents only when that client exposes them for this session.
"#;

const DEFAULT_AGENT_FLOW: &str = r#"Run the selected safe route directly. Use a resident search agent only when the active client exposes one for this session.
"#;

#[derive(Debug, Clone)]
pub(crate) struct CompiledRecoveryPromptConfig {
    template: Option<String>,
    codex_agent_flow: Option<String>,
    claude_agent_flow: Option<String>,
    default_agent_flow: Option<String>,
}

impl Default for CompiledRecoveryPromptConfig {
    fn default() -> Self {
        Self {
            template: Some(DEFAULT_RECOVERY_TEMPLATE.to_string()),
            codex_agent_flow: Some(CODEX_AGENT_FLOW.to_string()),
            claude_agent_flow: Some(CLAUDE_AGENT_FLOW.to_string()),
            default_agent_flow: Some(DEFAULT_AGENT_FLOW.to_string()),
        }
    }
}

impl CompiledRecoveryPromptConfig {
    pub(crate) fn template(&self) -> Option<&str> {
        self.template.as_deref()
    }

    pub(crate) fn agent_flow_for(&self, platform: &str) -> Option<&str> {
        if platform.eq_ignore_ascii_case("codex") {
            self.codex_agent_flow.as_deref()
        } else if platform.eq_ignore_ascii_case("claude") {
            self.claude_agent_flow.as_deref()
        } else {
            None
        }
        .or(self.default_agent_flow.as_deref())
    }
}

impl From<HookClientRecoveryPromptConfig> for CompiledRecoveryPromptConfig {
    fn from(config: HookClientRecoveryPromptConfig) -> Self {
        let defaults = Self::default();
        Self {
            template: config.template.or(defaults.template),
            codex_agent_flow: config.codex_agent_flow.or(defaults.codex_agent_flow),
            claude_agent_flow: config.claude_agent_flow.or(defaults.claude_agent_flow),
            default_agent_flow: config.default_agent_flow.or(defaults.default_agent_flow),
        }
    }
}
