// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use super::AgentActionKind;
use super::HostInvocationKind;

pub(crate) fn action_kind_matches(
    candidate: AgentActionKind,
    configured: agent_semantic_config::HookClientActionKind,
) -> bool {
    use agent_semantic_config::HookClientActionKind as Configured;
    matches!(
        (candidate, configured),
        (AgentActionKind::Read, Configured::Read)
            | (AgentActionKind::Search, Configured::Search)
            | (AgentActionKind::Edit, Configured::Edit)
            | (AgentActionKind::Execute, Configured::Execute)
            | (AgentActionKind::Mcp, Configured::Mcp)
            | (AgentActionKind::SpawnAgent, Configured::SpawnAgent)
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
            | (HostInvocationKind::Execute, Configured::Execute)
            | (HostInvocationKind::Mcp, Configured::Mcp)
            | (HostInvocationKind::SpawnAgent, Configured::SpawnAgent)
            | (HostInvocationKind::Unknown, Configured::Unknown)
    )
}
