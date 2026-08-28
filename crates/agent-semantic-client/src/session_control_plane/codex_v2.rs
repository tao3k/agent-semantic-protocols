use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum DurableChoiceAction {
    CreateAndRegister,
    Resume,
    NoAction,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum DurableAgentRunState {
    Active,
    Stopped,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DurableAgentNamespace {
    pub namespace_id: String,
    pub agent_name: String,
    pub root_session_id: String,
    pub parent_session_id: String,
    pub child_session_id: String,
    pub generation: u64,
    pub achieved: bool,
    pub run_state: DurableAgentRunState,
    pub last_host_event_id: String,
    pub last_host_event_sequence: u64,
    pub last_host_event_digest: String,
}

pub fn durable_choice(namespace: Option<&DurableAgentNamespace>) -> DurableChoiceAction {
    match namespace {
        None => DurableChoiceAction::CreateAndRegister,
        Some(namespace) if namespace.achieved => DurableChoiceAction::NoAction,
        Some(_) => DurableChoiceAction::Resume,
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum CodexV2HostEventKind {
    SubagentStart,
    SubagentResume,
    SubagentStop,
    SubagentAchieved,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CodexV2HostLifecycleEvent {
    pub host_event_id: String,
    pub host_event_sequence: u64,
    pub event_kind: CodexV2HostEventKind,
    pub root_session_id: String,
    pub parent_session_id: String,
    pub child_session_id: String,
    pub agent_name: String,
    pub namespace_id: String,
    pub observed_at_unix_ms: u64,
}

impl CodexV2HostLifecycleEvent {
    pub fn validate(&self) -> Result<(), &'static str> {
        if self.host_event_id.trim().is_empty()
            || self.root_session_id.trim().is_empty()
            || self.parent_session_id.trim().is_empty()
            || self.child_session_id.trim().is_empty()
            || self.namespace_id.trim().is_empty()
        {
            return Err("Codex v2 lifecycle identity fields must be non-empty");
        }
        if self.host_event_sequence == 0 {
            return Err("Codex v2 host event sequence must be positive");
        }
        if !self.agent_name.starts_with('@') || self.agent_name.len() == 1 {
            return Err("Codex v2 registered agent name must start with @");
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DurableMaterializationError {
    InvalidEvent(&'static str),
    NamespaceMissing,
    NamespaceIdentityMismatch,
    EventSequenceNotIncreasing,
    NamespaceAchieved,
    UnexpectedStartForExistingNamespace,
}

pub fn materialize_codex_v2_host_event(
    current: Option<&DurableAgentNamespace>,
    event: &CodexV2HostLifecycleEvent,
) -> Result<DurableAgentNamespace, DurableMaterializationError> {
    event
        .validate()
        .map_err(DurableMaterializationError::InvalidEvent)?;

    let event_digest = codex_v2_host_event_digest(event);
    match (current, event.event_kind) {
        (None, CodexV2HostEventKind::SubagentStart) => Ok(DurableAgentNamespace {
            namespace_id: event.namespace_id.clone(),
            agent_name: event.agent_name.clone(),
            root_session_id: event.root_session_id.clone(),
            parent_session_id: event.parent_session_id.clone(),
            child_session_id: event.child_session_id.clone(),
            generation: 1,
            achieved: false,
            run_state: DurableAgentRunState::Active,
            last_host_event_id: event.host_event_id.clone(),
            last_host_event_sequence: event.host_event_sequence,
            last_host_event_digest: event_digest,
        }),
        (None, _) => Err(DurableMaterializationError::NamespaceMissing),
        (Some(namespace), _) => {
            if namespace.namespace_id != event.namespace_id
                || namespace.agent_name != event.agent_name
                || namespace.root_session_id != event.root_session_id
                || namespace.parent_session_id != event.parent_session_id
                || namespace.child_session_id != event.child_session_id
            {
                return Err(DurableMaterializationError::NamespaceIdentityMismatch);
            }
            if event.host_event_sequence == namespace.last_host_event_sequence
                && event.host_event_id == namespace.last_host_event_id
                && event_digest == namespace.last_host_event_digest
            {
                return Ok(namespace.clone());
            }
            if event.host_event_sequence <= namespace.last_host_event_sequence {
                return Err(DurableMaterializationError::EventSequenceNotIncreasing);
            }
            if namespace.achieved {
                return Err(DurableMaterializationError::NamespaceAchieved);
            }
            if event.event_kind == CodexV2HostEventKind::SubagentStart {
                return Err(DurableMaterializationError::UnexpectedStartForExistingNamespace);
            }

            let mut next = namespace.clone();
            next.last_host_event_id = event.host_event_id.clone();
            next.last_host_event_sequence = event.host_event_sequence;
            next.last_host_event_digest = event_digest;
            match event.event_kind {
                CodexV2HostEventKind::SubagentResume => {
                    next.run_state = DurableAgentRunState::Active;
                }
                CodexV2HostEventKind::SubagentStop => {
                    next.run_state = DurableAgentRunState::Stopped;
                }
                CodexV2HostEventKind::SubagentAchieved => {
                    next.run_state = DurableAgentRunState::Stopped;
                    next.achieved = true;
                }
                CodexV2HostEventKind::SubagentStart => {
                    return Err(DurableMaterializationError::UnexpectedStartForExistingNamespace);
                }
            }
            Ok(next)
        }
    }
}

fn codex_v2_host_event_digest(event: &CodexV2HostLifecycleEvent) -> String {
    let bytes = serde_json::to_vec(event).expect("validated Codex v2 host event serializes");
    format!("blake3-256:{}", blake3::hash(&bytes).to_hex())
}

pub fn focused_child_admitted(
    parent_focused: bool,
    child_registered: bool,
    explicitly_declared: bool,
) -> bool {
    !parent_focused || child_registered || explicitly_declared
}

#[cfg(test)]
#[path = "../../tests/unit/session_control_plane_codex_v2.rs"]
mod tests;
