/// Runtime observations are root-scoped in host storage, so every lifecycle
/// consumer must partition them again by logical resident kind and physical
/// child generation before applying drift or Ready transitions.
pub(super) fn matches_resident_slot(
    expected_agent_type: &str,
    current_child_id: Option<&str>,
    observation_expected_agent_type: &str,
    observation_child_id: &str,
) -> bool {
    observation_expected_agent_type == expected_agent_type
        && current_child_id.is_none_or(|child_id| child_id == observation_child_id)
}

#[cfg(test)]
#[path = "../../tests/unit/agent_session_registry_observation.rs"]
mod tests;
