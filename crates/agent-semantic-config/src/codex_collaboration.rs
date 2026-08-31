use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

pub const CODEX_COLLABORATION_TOOL_CALL_SCHEMA_ID: &str =
    "agent.semantic-protocols.codex-collaboration-tool-call";
pub const CODEX_COLLABORATION_TOOL_CALL_SCHEMA_VERSION: &str = "1";
pub const CODEX_COLLABORATION_NAMESPACE: &str = "collaboration";

pub struct CodexMultiAgentV2Interface;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CodexCollaborationToolCall {
    pub schema_id: String,
    pub schema_version: String,
    pub namespace: String,
    #[serde(flatten)]
    pub operation: CodexCollaborationOperation,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "toolName", content = "toolInput", rename_all = "snake_case")]
pub enum CodexCollaborationOperation {
    SpawnAgent(SpawnAgentInput),
    ListAgents(ListAgentsInput),
    FollowupTask(MessageAgentInput),
    SendMessage(MessageAgentInput),
    InterruptAgent(TargetAgentInput),
    WaitAgent(WaitAgentInput),
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SpawnAgentInput {
    pub task_name: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub agent_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fork_turns: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reasoning_effort: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ListAgentsInput {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path_prefix: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct MessageAgentInput {
    pub target: String,
    pub message: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct TargetAgentInput {
    pub target: String,
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct WaitAgentInput {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timeout_ms: Option<i64>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CollaborationHostResultKind {
    SpawnedAgent,
    LiveAgents,
    EmptyActivity,
    PreviousStatus,
    WaitSummary,
}
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CollaborationLiveAgents {
    pub agents: Vec<CollaborationLiveAgent>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CollaborationLiveAgent {
    pub agent_name: String,
    pub agent_status: CollaborationAgentStatus,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CollaborationSpawnResult {
    pub task_name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub nickname: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CollaborationInterruptResult {
    pub previous_status: CollaborationAgentStatus,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CollaborationWaitResult {
    pub message: String,
    pub timed_out: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CollaborationAgentStatus {
    PendingInit,
    Running,
    Interrupted,
    Completed(Option<String>),
    Errored(String),
    Shutdown,
    NotFound,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CollaborationDispatchState {
    Absent,
    Running,
    Reusable,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CollaborationRegistrationState {
    MissingOrStale,
    Current,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CollaborationDispatchAction {
    SpawnAgentAndRegister,
    FollowupTaskAndRegister,
    FollowupTask,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CollaborationLifecycleTool {
    ListAgents,
    SpawnAgent,
    SendMessage,
    FollowupTask,
    InterruptAgent,
    WaitAgent,
}

impl CollaborationLifecycleTool {
    pub const fn starts_turn(self) -> bool {
        matches!(self, Self::SpawnAgent | Self::FollowupTask)
    }

    pub const fn observes_only(self) -> bool {
        matches!(self, Self::ListAgents | Self::WaitAgent)
    }

    pub const fn requires_existing_target(self) -> bool {
        matches!(
            self,
            Self::SendMessage | Self::FollowupTask | Self::InterruptAgent
        )
    }

    pub const fn preserves_existing_agent_path(self) -> bool {
        !matches!(self, Self::SpawnAgent)
    }

    pub const fn dispatches_required_work(self) -> bool {
        matches!(self, Self::SpawnAgent | Self::FollowupTask)
    }

    pub const fn host_result_kind(self) -> CollaborationHostResultKind {
        match self {
            Self::SpawnAgent => CollaborationHostResultKind::SpawnedAgent,
            Self::ListAgents => CollaborationHostResultKind::LiveAgents,
            Self::SendMessage | Self::FollowupTask => CollaborationHostResultKind::EmptyActivity,
            Self::InterruptAgent => CollaborationHostResultKind::PreviousStatus,
            Self::WaitAgent => CollaborationHostResultKind::WaitSummary,
        }
    }
}

impl CodexCollaborationToolCall {
    pub fn new(operation: CodexCollaborationOperation) -> Self {
        Self {
            schema_id: CODEX_COLLABORATION_TOOL_CALL_SCHEMA_ID.to_owned(),
            schema_version: CODEX_COLLABORATION_TOOL_CALL_SCHEMA_VERSION.to_owned(),
            namespace: CODEX_COLLABORATION_NAMESPACE.to_owned(),
            operation,
        }
    }

    pub const fn tool(&self) -> CollaborationLifecycleTool {
        match &self.operation {
            CodexCollaborationOperation::SpawnAgent(_) => CollaborationLifecycleTool::SpawnAgent,
            CodexCollaborationOperation::ListAgents(_) => CollaborationLifecycleTool::ListAgents,
            CodexCollaborationOperation::FollowupTask(_) => {
                CollaborationLifecycleTool::FollowupTask
            }
            CodexCollaborationOperation::SendMessage(_) => CollaborationLifecycleTool::SendMessage,
            CodexCollaborationOperation::InterruptAgent(_) => {
                CollaborationLifecycleTool::InterruptAgent
            }
            CodexCollaborationOperation::WaitAgent(_) => CollaborationLifecycleTool::WaitAgent,
        }
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.schema_id != CODEX_COLLABORATION_TOOL_CALL_SCHEMA_ID
            || self.schema_version != CODEX_COLLABORATION_TOOL_CALL_SCHEMA_VERSION
            || self.namespace != CODEX_COLLABORATION_NAMESPACE
        {
            return Err("Codex Collaboration tool-call identity is invalid".to_owned());
        }
        match &self.operation {
            CodexCollaborationOperation::SpawnAgent(input) => input.validate(),
            CodexCollaborationOperation::ListAgents(input) => input.validate(),
            CodexCollaborationOperation::FollowupTask(input) => {
                input.validate(/*allow_root*/ false)
            }
            CodexCollaborationOperation::SendMessage(input) => {
                input.validate(/*allow_root*/ true)
            }
            CodexCollaborationOperation::InterruptAgent(input) => {
                validate_target(&input.target, /*allow_root*/ false)
            }
            CodexCollaborationOperation::WaitAgent(input) => input.validate(),
        }
    }
}

impl CodexMultiAgentV2Interface {
    pub fn decode_tool_call(bytes: &[u8]) -> Result<CodexCollaborationToolCall, String> {
        let call: CodexCollaborationToolCall = serde_json::from_slice(bytes)
            .map_err(|error| format!("decode Codex Collaboration tool call: {error}"))?;
        call.validate()?;
        Ok(call)
    }

    pub fn decode_live_agents(bytes: &[u8]) -> Result<CollaborationLiveAgents, String> {
        let agents: CollaborationLiveAgents = serde_json::from_slice(bytes)
            .map_err(|error| format!("decode collaboration.list_agents result: {error}"))?;
        agents.validate_root_tree()?;
        Ok(agents)
    }

    pub fn choose_dispatch(
        agents: &CollaborationLiveAgents,
        canonical_path: &str,
        registration: CollaborationRegistrationState,
    ) -> Result<CollaborationDispatchAction, String> {
        agents.dispatch_action(canonical_path, registration)
    }
}

impl SpawnAgentInput {
    fn validate(&self) -> Result<(), String> {
        if !is_task_name(&self.task_name) {
            return Err(
                "spawn_agent task_name must use lowercase letters, digits, or underscores"
                    .to_owned(),
            );
        }
        require_non_blank("spawn_agent message", &self.message)?;
        for (field, value) in [
            ("agent_type", self.agent_type.as_deref()),
            ("model", self.model.as_deref()),
            ("reasoning_effort", self.reasoning_effort.as_deref()),
        ] {
            if let Some(value) = value {
                require_non_blank(field, value)?;
            }
        }
        if let Some(fork_turns) = self.fork_turns.as_deref()
            && fork_turns != "none"
            && fork_turns != "all"
            && !fork_turns.parse::<usize>().is_ok_and(|turns| turns > 0)
        {
            return Err(
                "spawn_agent fork_turns must be `none`, `all`, or a positive integer string"
                    .to_owned(),
            );
        }
        Ok(())
    }
}

impl ListAgentsInput {
    fn validate(&self) -> Result<(), String> {
        if let Some(prefix) = self.path_prefix.as_deref()
            && (!is_canonical_agent_path(prefix) || prefix.ends_with('/'))
        {
            return Err(
                "list_agents path_prefix must be a canonical Agent path without a trailing slash"
                    .to_owned(),
            );
        }
        Ok(())
    }
}

impl MessageAgentInput {
    fn validate(&self, allow_root: bool) -> Result<(), String> {
        validate_target(&self.target, allow_root)?;
        require_non_blank("Collaboration message", &self.message)
    }
}

impl WaitAgentInput {
    fn validate(&self) -> Result<(), String> {
        if let Some(timeout_ms) = self.timeout_ms
            && !(10_000..=3_600_000).contains(&timeout_ms)
        {
            return Err("wait_agent timeout_ms must be between 10000 and 3600000".to_owned());
        }
        Ok(())
    }
}

pub const fn state_after_interrupt(
    state: CollaborationDispatchState,
) -> CollaborationDispatchState {
    match state {
        CollaborationDispatchState::Absent => CollaborationDispatchState::Absent,
        CollaborationDispatchState::Running | CollaborationDispatchState::Reusable => {
            CollaborationDispatchState::Reusable
        }
    }
}

impl CollaborationLiveAgents {
    pub fn validate(&self) -> Result<(), String> {
        let mut paths = BTreeSet::new();
        for agent in &self.agents {
            if !is_canonical_agent_path(&agent.agent_name) {
                return Err(format!(
                    "collaboration.list_agents returned a non-canonical path: {}",
                    agent.agent_name
                ));
            }
            if !paths.insert(agent.agent_name.as_str()) {
                return Err(format!(
                    "collaboration.list_agents returned a duplicate path: {}",
                    agent.agent_name
                ));
            }
            if agent.agent_status == CollaborationAgentStatus::NotFound {
                return Err(format!(
                    "collaboration.list_agents returned a not_found entry for a live path: {}",
                    agent.agent_name
                ));
            }
        }
        Ok(())
    }

    pub fn validate_root_tree(&self) -> Result<(), String> {
        self.validate()?;
        if !self.agents.iter().any(|agent| agent.agent_name == "/root") {
            return Err(
                "collaboration.list_agents must include the current /root Agent".to_owned(),
            );
        }
        Ok(())
    }

    pub fn dispatch_state(
        &self,
        canonical_path: &str,
    ) -> Result<CollaborationDispatchState, String> {
        self.validate_root_tree()?;
        let Some(agent) = self
            .agents
            .iter()
            .find(|agent| agent.agent_name == canonical_path)
        else {
            return Ok(CollaborationDispatchState::Absent);
        };
        Ok(
            if matches!(
                agent.agent_status,
                CollaborationAgentStatus::Running | CollaborationAgentStatus::PendingInit
            ) {
                CollaborationDispatchState::Running
            } else {
                CollaborationDispatchState::Reusable
            },
        )
    }

    pub fn dispatch_action(
        &self,
        canonical_path: &str,
        registration: CollaborationRegistrationState,
    ) -> Result<CollaborationDispatchAction, String> {
        Ok(match (self.dispatch_state(canonical_path)?, registration) {
            (CollaborationDispatchState::Absent, _) => {
                CollaborationDispatchAction::SpawnAgentAndRegister
            }
            (
                CollaborationDispatchState::Running | CollaborationDispatchState::Reusable,
                CollaborationRegistrationState::Current,
            ) => CollaborationDispatchAction::FollowupTask,
            (
                CollaborationDispatchState::Running | CollaborationDispatchState::Reusable,
                CollaborationRegistrationState::MissingOrStale,
            ) => CollaborationDispatchAction::FollowupTaskAndRegister,
        })
    }
}

impl CollaborationSpawnResult {
    pub fn validate(&self) -> Result<(), String> {
        require_non_blank("spawn_agent result task_name", &self.task_name)?;
        if let Some(nickname) = self.nickname.as_deref() {
            require_non_blank("spawn_agent result nickname", nickname)?;
        }
        Ok(())
    }
}

impl CollaborationWaitResult {
    pub fn validate(&self) -> Result<(), String> {
        require_non_blank("wait_agent result message", &self.message)
    }
}

fn is_canonical_agent_path(path: &str) -> bool {
    if path == "/root" {
        return true;
    }
    let Some(tail) = path.strip_prefix("/root/") else {
        return false;
    };
    !tail.is_empty()
        && tail.split('/').all(|part| {
            !part.is_empty()
                && part
                    .bytes()
                    .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_')
        })
}

fn validate_target(target: &str, allow_root: bool) -> Result<(), String> {
    require_non_blank("Collaboration target", target)?;
    if target == "/root" && !allow_root {
        return Err("this Collaboration operation cannot target the root Agent".to_owned());
    }
    Ok(())
}

fn require_non_blank(field: &str, value: &str) -> Result<(), String> {
    if value.trim().is_empty() {
        return Err(format!("{field} must be non-blank"));
    }
    Ok(())
}

fn is_task_name(value: &str) -> bool {
    !value.is_empty()
        && value
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_')
}

#[cfg(test)]
#[path = "../tests/unit/codex_collaboration.rs"]
mod tests;
