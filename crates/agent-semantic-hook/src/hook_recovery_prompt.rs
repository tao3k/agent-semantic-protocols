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

const CODEX_AGENT_FLOW: &str = r#"Codex: select the semantic lifecycle action from the Runtime-owned AgentSession Registry disposition; never infer registration from tool availability, Host status, task creation, or empty output.
This contract requires the current Host capability snapshot to contain `multi_agent_v2` and its complete `collaboration.*` tool family. Bind the selected action to that exact v2 tool. Never fall back to a legacy agent tool family, thread APIs, prose substitution, or a partial capability family. If v2 is absent, return exactly one typed failure with `reasonKind=codex-multi-agent-v2-capability-unavailable`; do not spawn, retry, or claim that the selected Registry action was executed. The renderer must never emit a tool name that is not callable.
If the exact parent-child binding is current, use `collaboration.followup_task` on the existing canonical Agent path. If the Host path exists but its binding is absent or stale, use `collaboration.followup_task` to register that same child exactly once before the denied operation. Only an absent Registry binding plus an absent Host path may use `collaboration.spawn_agent` with the Config-resolved `agent_type`.
Treat an accepted spawn as `bootstrap-pending`, not registered. Require exactly one native Host receipt carrying the typed registration terminal before ordinary follow-up. While the Host task is running, `state=empty-payload` remains pending. If that Host task completes or errors without a typed bootstrap terminal, emit `reasonKind=agent-bootstrap-terminal-missing` exactly once and repair that same child; do not respawn or classify Registry registration as rejected.
Use `collaboration.list_agents({ path_prefix: \"/root\" })` only when that exact operation is injected and only to distinguish an absent Host path from an existing unregistered path, or for post-action observation. It is never the first unconditional dispatch action. If no path-observation operation is callable and path presence is unknown, fail closed with `reasonKind=host-agent-observation-capability-unavailable`; do not infer absence and do not spawn.
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
            // Collaboration dispatch is a Runtime/Registry lifecycle decision, not
            // configurable recovery prose. Keep parsing the legacy field for V1
            // compatibility, but never allow it to replace the fail-closed flow.
            codex_agent_flow: defaults.codex_agent_flow,
            claude_agent_flow: config.claude_agent_flow.or(defaults.claude_agent_flow),
            default_agent_flow: config.default_agent_flow.or(defaults.default_agent_flow),
        }
    }
}

#[cfg(test)]
#[path = "../tests/unit/hook_recovery_prompt.rs"]
mod tests;
