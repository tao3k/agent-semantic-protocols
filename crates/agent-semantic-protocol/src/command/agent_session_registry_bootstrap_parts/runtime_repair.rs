use agent_semantic_client_db::agent_session_registry::AgentSessionInteractiveMenu;

pub(super) fn main_agent_runtime_rebind_instruction(
    canonical_target: &str,
    observation: &agent_semantic_hook::SubagentRuntimeDriftObservation,
    managed_agent_kind: &str,
) -> String {
    format!(
        "Retire/archive drifted target {canonical_target} and child {}, wait for terminal host status and canonical-path release, then create exactly one replacement through a spawn surface that exposes agent_type, with agent_type={managed_agent_kind}, task_name={managed_agent_kind}, and fork_turns=none. Message/natural-language task text is only payload and does not select the registered role. Codex must load the complete registered TOML profile; if agent_type is unavailable, report the host capability blocker instead of spawning.",
        observation.child_session_id,
    )
}

pub(super) fn runtime_switch_followup_message(menu: &AgentSessionInteractiveMenu<'_>) -> String {
    format!(
        "Retire the drifted resident and create one typed {} replacement from the registered Codex role. The expected runtime is model {} with reasoning {}, but Codex must obtain all values from the role TOML rather than this message.",
        menu.host_requirement.managed_agent_kind.as_ref(),
        menu.expected_model.unwrap_or("unknown"),
        menu.expected_reasoning_effort.unwrap_or("unknown"),
    )
}

pub(super) struct RuntimeRepairDiagnosis {
    pub(super) drift_dimensions: Vec<&'static str>,
    pub(super) bootstrap_blocked: &'static str,
    pub(super) repair_attempt_status: &'static str,
}

pub(super) fn runtime_repair_diagnosis(
    menu: &AgentSessionInteractiveMenu<'_>,
    observation: &agent_semantic_hook::SubagentRuntimeDriftObservation,
) -> RuntimeRepairDiagnosis {
    let model_drift = menu
        .expected_model
        .is_some_and(|expected| observation.observed_model.as_deref() != Some(expected));
    let reasoning_drift = menu
        .expected_reasoning_effort
        .is_some_and(|expected| observation.observed_reasoning_effort.as_deref() != Some(expected));
    match (model_drift, reasoning_drift) {
        (false, true) => RuntimeRepairDiagnosis {
            drift_dimensions: vec!["reasoningEffort"],
            bootstrap_blocked: "host-typed-resident-replacement-unavailable",
            repair_attempt_status: "typed-resident-replacement-required",
        },
        (true, false) => RuntimeRepairDiagnosis {
            drift_dimensions: vec!["model"],
            bootstrap_blocked: "host-typed-resident-replacement-unavailable",
            repair_attempt_status: "typed-resident-replacement-required",
        },
        _ => RuntimeRepairDiagnosis {
            drift_dimensions: vec!["model", "reasoningEffort"],
            bootstrap_blocked: "host-typed-resident-replacement-unavailable",
            repair_attempt_status: "typed-resident-replacement-required",
        },
    }
}

pub(super) fn latest_trace_result(menu: &AgentSessionInteractiveMenu<'_>) -> String {
    menu.trace
        .last()
        .map(|step| step.result)
        .or(menu.rollout_history_status)
        .unwrap_or("loop state requires action")
        .to_string()
}
