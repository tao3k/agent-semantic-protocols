// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use serde::Deserialize;
use serde::Serialize;

pub const AGENT_SESSION_LIFECYCLE_PROJECTION_SCHEMA_ID: &str =
    "agent.semantic-protocols.agent-session-lifecycle-projection";
pub const AGENT_SESSION_LIFECYCLE_PROJECTION_SCHEMA_VERSION: &str = "1";

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ServerHealth {
    Unobserved,
    Ready,
    Repairing,
    Unavailable,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum SessionPhase {
    Unobserved,
    Declared,
    Active,
    Interrupted,
    Completed,
    Failed,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum BindingPhase {
    Unobserved,
    Unbound,
    Fresh,
    Stale,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum DispatchPhase {
    Unobserved,
    Idle,
    Claimed,
    Running,
    Quarantined,
    Completed,
    Rejected,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceServerProjection {
    pub workspace_identity: String,
    pub health: ServerHealth,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionLifecycleProjection {
    pub session_id: Option<String>,
    pub generation: Option<u64>,
    pub phase: SessionPhase,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HostBindingProjection {
    pub generation: Option<u64>,
    pub phase: BindingPhase,
    pub child_session_id: Option<String>,
    pub canonical_message_target: Option<String>,
    pub path_observed: Option<bool>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum RequiredDispatchAction {
    Unavailable,
    SpawnAgent,
    FollowupTask,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DispatchLifecycleProjection {
    pub generation: Option<u64>,
    pub phase: DispatchPhase,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentSessionLifecycleProjection {
    pub schema_id: String,
    pub schema_version: String,
    pub workspace_server: WorkspaceServerProjection,
    pub session: SessionLifecycleProjection,
    pub host_binding: HostBindingProjection,
    pub dispatch: DispatchLifecycleProjection,
    pub required_dispatch_action: RequiredDispatchAction,
    pub durable_dispatch_authorized: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HostBindingObservation {
    Unobserved,
    PresentFresh,
    PresentStale,
    Absent,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DispatchObservation {
    Unobserved,
    Idle,
    Claimed,
    Running,
    Quarantined,
    Completed,
    Rejected,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HostBindingFacts {
    pub recorded: bool,
    pub generation: Option<u64>,
    pub child_session_id: Option<String>,
    pub canonical_message_target: Option<String>,
    pub observation: HostBindingObservation,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AgentSessionLifecycleFacts {
    pub workspace_identity: String,
    pub server_health: ServerHealth,
    pub session_id: Option<String>,
    pub session_generation: Option<u64>,
    pub registry_status: Option<String>,
    pub host_binding: HostBindingFacts,
    pub dispatch_generation: Option<u64>,
    pub dispatch_observation: DispatchObservation,
}

pub fn session_phase_from_registry_status(status: Option<&str>) -> Result<SessionPhase, String> {
    match status {
        None => Ok(SessionPhase::Unobserved),
        Some("declared") => Ok(SessionPhase::Declared),
        Some("active") => Ok(SessionPhase::Active),
        Some("interrupted") => Ok(SessionPhase::Interrupted),
        Some("completed") => Ok(SessionPhase::Completed),
        Some("failed") => Ok(SessionPhase::Failed),
        Some(status @ ("idle" | "invalid" | "orphan-risk" | "archived" | "retired")) => Err(
            format!("registry status `{status}` conflates session, binding, or dispatch lifecycle"),
        ),
        Some(status) => Err(format!(
            "unsupported agent session registry status `{status}`"
        )),
    }
}

pub fn project_host_binding(facts: HostBindingFacts) -> Result<HostBindingProjection, String> {
    if facts.recorded && facts.generation.is_none() {
        return Err("a recorded host binding requires a physical generation".to_owned());
    }
    if facts.recorded
        && (facts.child_session_id.as_deref().is_none_or(str::is_empty)
            || facts
                .canonical_message_target
                .as_deref()
                .is_none_or(str::is_empty))
    {
        return Err(
            "a recorded host binding requires child session id and canonical message target"
                .to_owned(),
        );
    }

    let phase = if !facts.recorded {
        BindingPhase::Unbound
    } else {
        match facts.observation {
            HostBindingObservation::Unobserved => BindingPhase::Unobserved,
            HostBindingObservation::PresentFresh => BindingPhase::Fresh,
            HostBindingObservation::PresentStale | HostBindingObservation::Absent => {
                BindingPhase::Stale
            }
        }
    };
    Ok(HostBindingProjection {
        generation: facts.generation,
        phase,
        child_session_id: facts.child_session_id,
        canonical_message_target: facts.canonical_message_target,
        path_observed: match facts.observation {
            HostBindingObservation::Unobserved => None,
            HostBindingObservation::PresentFresh | HostBindingObservation::PresentStale => {
                Some(true)
            }
            HostBindingObservation::Absent => Some(false),
        },
    })
}

pub fn project_dispatch(
    generation: Option<u64>,
    observation: DispatchObservation,
) -> DispatchLifecycleProjection {
    let phase = match observation {
        DispatchObservation::Unobserved => DispatchPhase::Unobserved,
        DispatchObservation::Idle => DispatchPhase::Idle,
        DispatchObservation::Claimed => DispatchPhase::Claimed,
        DispatchObservation::Running => DispatchPhase::Running,
        DispatchObservation::Quarantined => DispatchPhase::Quarantined,
        DispatchObservation::Completed => DispatchPhase::Completed,
        DispatchObservation::Rejected => DispatchPhase::Rejected,
    };
    DispatchLifecycleProjection { generation, phase }
}

pub fn project_agent_session_lifecycle(
    facts: AgentSessionLifecycleFacts,
) -> Result<AgentSessionLifecycleProjection, String> {
    if facts.workspace_identity.is_empty() {
        return Err("workspace identity must not be empty".to_owned());
    }
    if facts.session_id.as_deref().is_some_and(str::is_empty) {
        return Err("session id must not be empty".to_owned());
    }
    let session_phase = session_phase_from_registry_status(facts.registry_status.as_deref())?;
    if session_phase == SessionPhase::Unobserved {
        if facts.session_generation.is_some() {
            return Err("an unobserved session cannot claim a generation".to_owned());
        }
    } else if facts.session_id.is_none() || facts.session_generation.is_none() {
        return Err("an observed session requires identity and generation".to_owned());
    }
    if facts.dispatch_observation != DispatchObservation::Unobserved
        && facts.dispatch_generation.is_none()
    {
        return Err("an observed dispatch requires a generation".to_owned());
    }
    let workspace_server = WorkspaceServerProjection {
        workspace_identity: facts.workspace_identity,
        health: facts.server_health,
    };
    let session = SessionLifecycleProjection {
        session_id: facts.session_id,
        generation: facts.session_generation,
        phase: session_phase,
    };
    let host_binding = project_host_binding(facts.host_binding)?;
    let dispatch = project_dispatch(facts.dispatch_generation, facts.dispatch_observation);
    Ok(AgentSessionLifecycleProjection::new(
        workspace_server,
        session,
        host_binding,
        dispatch,
    ))
}

impl AgentSessionLifecycleProjection {
    pub fn new(
        workspace_server: WorkspaceServerProjection,
        session: SessionLifecycleProjection,
        host_binding: HostBindingProjection,
        dispatch: DispatchLifecycleProjection,
    ) -> Self {
        let required_dispatch_action = match host_binding.path_observed {
            Some(true) => RequiredDispatchAction::FollowupTask,
            Some(false) => RequiredDispatchAction::SpawnAgent,
            None => RequiredDispatchAction::Unavailable,
        };
        let durable_dispatch_authorized = session.phase == SessionPhase::Active
            && host_binding.phase == BindingPhase::Fresh
            && host_binding.path_observed == Some(true)
            && session.generation.is_some()
            && host_binding.generation == session.generation
            && dispatch.generation == session.generation;
        Self {
            schema_id: AGENT_SESSION_LIFECYCLE_PROJECTION_SCHEMA_ID.to_owned(),
            schema_version: AGENT_SESSION_LIFECYCLE_PROJECTION_SCHEMA_VERSION.to_owned(),
            workspace_server,
            session,
            host_binding,
            dispatch,
            required_dispatch_action,
            durable_dispatch_authorized,
        }
    }

    pub fn followup_task_admitted(&self) -> bool {
        self.required_dispatch_action == RequiredDispatchAction::FollowupTask
            && self.host_binding.phase == BindingPhase::Fresh
            && self.host_binding.generation == self.session.generation
    }

    pub fn spawn_agent_admitted(&self) -> bool {
        self.required_dispatch_action == RequiredDispatchAction::SpawnAgent
    }
}
