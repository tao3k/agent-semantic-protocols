//! Provider-neutral state transition for the Host agent ChoicePlane.

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum HostLifecycleSurface {
    Available,
    Unavailable,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct AgentSessionChoiceState<'a> {
    pub state: &'a str,
    pub reason_kind: Option<&'a str>,
}

/// Resolve the agent-session choice without consulting workspace or provider generations.
///
/// Those generations are deliberately absent from this interface: a Host lifecycle action is
/// the prerequisite for, rather than a consumer of, workspace/provider admission.
pub(crate) fn resolve_agent_session_choice_state<'a>(
    namespace_state: &'a str,
    namespace_reason_kind: Option<&'a str>,
    host_surface: HostLifecycleSurface,
) -> AgentSessionChoiceState<'a> {
    if namespace_state == "registration-required"
        && host_surface == HostLifecycleSurface::Unavailable
    {
        return AgentSessionChoiceState {
            state: "host-surface-unavailable",
            reason_kind: Some("host-lifecycle-surface-unavailable"),
        };
    }
    AgentSessionChoiceState {
        state: namespace_state,
        reason_kind: namespace_reason_kind,
    }
}

#[cfg(test)]
#[path = "../tests/unit/agent_session_choice_state.rs"]
mod tests;
