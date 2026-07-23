//! Enforces configured resident dispatch and terminal wrapper contracts.

/// Deny configured resident spawns that do not use their canonical isolated context.
pub(super) fn enforce_configured_resident_spawn_contract(
    hook_config: &agent_semantic_hook::ClientHookConfig,
    platform: &str,
    event: &str,
    payload: &serde_json::Value,
    decision: &mut agent_semantic_hook::HookDecision,
) {
    if platform != "codex" || event != "pre-tool" {
        return;
    }
    let Some(tool_name) = payload.get("tool_name").and_then(serde_json::Value::as_str) else {
        return;
    };
    if tool_name.rsplit(['.', ':']).next() != Some("spawn_agent") {
        return;
    }
    let Some(tool_input) = payload.get("tool_input") else {
        return;
    };
    let Some(agent_type) = tool_input
        .get("agent_type")
        .and_then(serde_json::Value::as_str)
    else {
        return;
    };
    let Some(target) = hook_config.configured_resident_target(agent_type) else {
        return;
    };
    let task_name = tool_input
        .get("task_name")
        .and_then(serde_json::Value::as_str);
    let fork_turns = tool_input
        .get("fork_turns")
        .and_then(serde_json::Value::as_str);
    if task_name == Some(target.codex_agent_name) && fork_turns == Some("none") {
        return;
    }
    decision.decision = agent_semantic_hook::DecisionKind::Deny;
    decision.reason_kind = agent_semantic_hook::ReasonKind::None;
    decision.message = format!(
        "Configured resident `{}` must be spawned through its isolated canonical context.",
        target.resident_name
    );
    for (key, value) in [
        (
            "requiredAction",
            "spawn-configured-resident-with-isolated-context",
        ),
        ("residentChildName", target.resident_name),
        ("targetAgentName", target.codex_agent_name),
        ("targetAgentRole", target.role),
        ("requiredForkTurns", "none"),
    ] {
        decision.fields.insert(
            key.to_string(),
            serde_json::Value::String(value.to_string()),
        );
    }
    decision.fields.insert(
        "canonicalTarget".to_string(),
        serde_json::Value::String(format!("/root/{}", target.codex_agent_name)),
    );
}

/// Mark the validating dispatch wrapper as a terminal resident execution bridge.
pub(super) fn materialize_resident_dispatch_wrapper(
    payload: &serde_json::Value,
    decision: &mut agent_semantic_hook::HookDecision,
) {
    if decision.decision != agent_semantic_hook::DecisionKind::Allow {
        return;
    }
    let Some(command) = payload
        .get("tool_input")
        .and_then(|input| input.get("command"))
        .and_then(serde_json::Value::as_str)
    else {
        return;
    };
    let tokens = agent_semantic_hook::semantic_shell_tokens(command);
    if !tokens
        .windows(4)
        .any(|window| window == ["asp", "agent", "session", "dispatch-execute"])
    {
        return;
    }
    decision.fields.insert(
        "agentSessionAction".to_string(),
        serde_json::Value::String("resident-command-bridge".to_string()),
    );
    super::hook_runtime_agent_session::append_terminal_execution_fields(
        &mut decision.fields,
        "resident-command-bridge",
    );
}
