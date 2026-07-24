//! Host-profile selection and unique resident-child dispatch policy.

use super::AspSessionPolicy;
use std::collections::BTreeMap;

pub(super) fn append_resident_agent_fields(
    fields: &mut BTreeMap<String, serde_json::Value>,
    platform: &str,
    asp_session_policy: &AspSessionPolicy,
) {
    fields.insert(
        "targetAgentName".to_string(),
        serde_json::Value::String(asp_session_policy.resident_codex_agent_name().to_string()),
    );
    fields.insert(
        "targetAgentRole".to_string(),
        serde_json::Value::String(asp_session_policy.resident_agent_role().to_string()),
    );
    fields.insert(
        "residentCodexAgentName".to_string(),
        serde_json::Value::String(asp_session_policy.resident_codex_agent_name().to_string()),
    );
    fields.insert(
        "targetAgentSelectionSource".to_string(),
        serde_json::Value::String("hook-deny-intent".to_string()),
    );
    fields.insert(
        "targetAgentRegistrySource".to_string(),
        serde_json::Value::String("~/.agent-semantic-protocols/agents/config.toml".to_string()),
    );
    fields.insert(
        "targetAgentLifecycleOwner".to_string(),
        serde_json::Value::String("main-agent".to_string()),
    );
    fields.insert(
        "targetAgentReusePolicy".to_string(),
        serde_json::Value::String("reuse-unique-child-before-spawn".to_string()),
    );
    if platform == "codex" {
        fields.insert(
            "targetAgentHostRegistry".to_string(),
            serde_json::Value::String("~/.codex/agents".to_string()),
        );
        fields.insert(
            "targetAgentDispatchPolicy".to_string(),
            serde_json::Value::String(
                "followup-idle-or-completed-send-message-running-spawn-only-if-absent".to_string(),
            ),
        );
    }
}

pub(super) fn reasoning_observation_mismatches_profile(
    observed: Option<&str>,
    expected: Option<&str>,
) -> bool {
    observed.is_some_and(|observed| expected.is_some_and(|expected| observed != expected))
}

pub(super) fn typed_spawn_reasoning_verification(
    observed: Option<&str>,
    configured: Option<&str>,
) -> serde_json::Value {
    serde_json::json!({
        "observedReasoningEffort": observed,
        "configuredReasoningEffort": configured,
        "status": if observed.is_some() {
            "observed-match"
        } else {
            "typed-spawn-profile-attested"
        },
        "source": if observed.is_some() {
            "codex.subagent-start-or-rollout"
        } else {
            "asp-collaboration-spawn-plus-profile"
        },
    })
}

pub(super) fn resident_child_create_action(
    platform: &str,
    asp_session_policy: &AspSessionPolicy,
) -> String {
    match platform {
        "codex" => format!(
            "Codex action: start the configured ASP managed subagent `{}` only through an `agent_type`-aware native spawn surface, with `agent_type={}` and `fork_turns=none`. `task_name` controls tree identity and `message` is only task payload; neither selects the custom-agent TOML. If `agent_type` is absent from the host surface, report bootstrapBlocked=host-agent-type-unavailable and do not create a generic subagent. After a valid SubagentStart, send the exact denied ASP command to that same child and wait for a bounded `[asp-search-subagent]` receipt.",
            asp_session_policy.resident_codex_agent_name(),
            asp_session_policy.resident_codex_agent_name()
        ),
        "claude" => "Claude action: start the configured subagent `asp-explorer`".to_string(),
        _ => "Host action: start the configured resident ASP explore subagent".to_string(),
    }
}
