use super::AgentActionKind;

pub(crate) fn action_kind_matches(
    candidate: AgentActionKind,
    configured: agent_semantic_config::HookClientActionKind,
) -> bool {
    use agent_semantic_config::HookClientActionKind as Configured;
    matches!(
        (candidate, configured),
        (AgentActionKind::Read, Configured::Read)
            | (AgentActionKind::Edit, Configured::Edit)
            | (AgentActionKind::Search, Configured::Search)
            | (AgentActionKind::Enumerate, Configured::Enumerate)
            | (AgentActionKind::Execute, Configured::Execute)
            | (AgentActionKind::Unknown, Configured::Unknown)
    )
}
