//! Compiled recovery prompt fragments for hook-deny guidance.

use agent_semantic_config::HookClientRecoveryPromptConfig;

const DEFAULT_RECOVERY_TEMPLATE: &str = r#"ASP denied `{reason}`. Do not retry raw source tools.
Use the ASP route below, or delegate the lookup through the configured registered Agent when available.
Return one compact `[asp-search-subagent]` graph-route receipt with `schema`, `intent`, `route`, `state`, ranked selector `evidence`, and exactly one safe parent `next` action.
Do not run raw source tools again for this step. The file extension maps to the
`languageId` in the route; use that language and the exact command below, then follow `next` from the same payload.
```sh
<command>
```
{routes}
{agent_flow}
"#;

const CODEX_AGENT_FLOW: &str = r#"Codex: use `collaboration.spawn_agent` with the Config-resolved `agent_type` for ASP search/query work.
Verify the returned canonical agent path with `collaboration.list_agents`, and require the native Host receipt before running the route.
After the route command succeeds, continue exactly with the returned `next` action; no ASP lifecycle command is needed.
If denied repeatedly, do not switch to raw shell sources. Run the `next` action from this payload only after the route command completes.
"#;

const CLAUDE_AGENT_FLOW: &str = r#"Claude: invoke the Config-resolved registered Agent with Claude's native Agent tool or `@agent-<name>` mention, then run the selected safe route there.
"#;

const DEFAULT_AGENT_FLOW: &str = r#"Run the selected safe route directly. Use the configured registered Agent only when the active client exposes it for this session.
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

#[cfg(test)]
#[path = "../tests/unit/hook_recovery_prompt.rs"]
mod tests;
