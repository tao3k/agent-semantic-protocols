use super::{AgentActionKind, HostInvocationKind};

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

pub(crate) fn host_invocation_kind_matches(
    candidate: HostInvocationKind,
    configured: agent_semantic_config::HookClientHostInvocationKind,
) -> bool {
    use agent_semantic_config::HookClientHostInvocationKind as Configured;
    matches!(
        (candidate, configured),
        (HostInvocationKind::Read, Configured::Read)
            | (HostInvocationKind::Edit, Configured::Edit)
            | (HostInvocationKind::Search, Configured::Search)
            | (HostInvocationKind::Enumerate, Configured::Enumerate)
            | (HostInvocationKind::Execute, Configured::Execute)
            | (HostInvocationKind::Mcp, Configured::Mcp)
            | (HostInvocationKind::Unknown, Configured::Unknown)
    )
}
