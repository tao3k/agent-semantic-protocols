use agent_semantic_hook::{DecisionKind, HookDecision, ReasonKind};
use std::path::Path;

/// Treat host-observed resident drift as lifecycle diagnosis, never as
/// authority to seal the parent task's tool surface.
pub(super) fn soften_drifted_resident_route(
    project_root: &Path,
    decision: &mut HookDecision,
) -> Result<(), String> {
    if decision.decision != DecisionKind::Deny {
        return Ok(());
    }
    let root_session_id = decision
        .fields
        .get("rootSessionId")
        .or_else(|| decision.fields.get("sessionId"))
        .and_then(serde_json::Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .map(str::to_string)
        .or_else(|| {
            std::env::var("CODEX_THREAD_ID")
                .ok()
                .filter(|value| !value.trim().is_empty())
        });
    let Some(root_session_id) = root_session_id.as_deref() else {
        return Ok(());
    };
    let Some(observation) =
        agent_semantic_hook::latest_subagent_runtime_drift(project_root, root_session_id)?
    else {
        return Ok(());
    };
    // This API only returns observations previously classified as drift; the
    // expected runtime remains owned by the registered Codex role.
    decision.decision = DecisionKind::Allow;
    decision.reason_kind = ReasonKind::None;
    decision.message = format!(
        "ASP resident routing is degraded because child {} was observed with agent_type `{}` instead of `{}` or with a mismatched runtime. Allowing this Codex tool call; retire the drifted child and create one replacement through the registered `asp_explorer` role when the host exposes typed replacement.",
        observation.child_session_id,
        observation.observed_agent_type,
        observation.expected_agent_type,
    );
    clear_bootstrap_loop_fields(decision);
    decision.fields.insert(
        "residentRouteStatus".to_string(),
        serde_json::Value::String("degraded-profile-or-runtime-drift".to_string()),
    );
    decision.fields.insert(
        "residentRoutePolicy".to_string(),
        serde_json::Value::String("soft-nonblocking".to_string()),
    );
    decision.fields.insert(
        "residentReplacementAction".to_string(),
        serde_json::Value::String(
            "retire-and-create-agent-type-asp-explorer-from-registered-toml".to_string(),
        ),
    );
    Ok(())
}

fn clear_bootstrap_loop_fields(decision: &mut HookDecision) {
    [
        "requiredAction",
        "nextAction",
        "forbiddenUntilResolved",
        "agentSessionLoopCommand",
        "agentSessionBootstrap",
        "agentSessionBootstrapGuideCommand",
        "agentSessionBootstrapCommand",
    ]
    .into_iter()
    .for_each(|field| {
        decision.fields.remove(field);
    });
}
