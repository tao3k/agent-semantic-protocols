use serde::{Deserialize, Serialize};

pub const AGENT_SESSION_NAMESPACE_PROJECTION_SCHEMA_ID: &str =
    "agent.semantic-protocols.agent-session-namespace-projection";
pub const AGENT_SESSION_NAMESPACE_PROJECTION_SCHEMA_VERSION: &str = "1";

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum DurableNamespaceState {
    Absent,
    Present,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum HostChildLiveness {
    Running,
    Idle,
    Terminated,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ChoicePlaneDecision {
    CreateNew,
    ContinueExisting,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum NamespaceAction {
    Create,
    Resume,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum CodexHostAction {
    #[serde(rename = "spawn_agent")]
    SpawnAgent,
    #[serde(rename = "followup_task")]
    FollowupTask,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum SessionIdentityAuthority {
    PlatformEnvironment,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PlatformSessionIdentity {
    pub platform: String,
    pub platform_session_id: String,
    pub session_identity_authority: SessionIdentityAuthority,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct AgentSessionNamespaceKey {
    pub project_id: String,
    pub workspace_id: String,
    pub canonical_workspace_root: String,
    pub platform: String,
    pub platform_session_id: String,
    pub root_namespace_id: String,
    pub agent_name: String,
}

impl PlatformSessionIdentity {
    pub fn from_platform_environment(
        platform: impl Into<String>,
        platform_session_id: impl Into<String>,
    ) -> Self {
        Self {
            platform: platform.into(),
            platform_session_id: platform_session_id.into(),
            session_identity_authority: SessionIdentityAuthority::PlatformEnvironment,
        }
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.platform.trim().is_empty() {
            return Err("agent session platform must be non-empty".to_owned());
        }
        if self.platform_session_id.trim().is_empty() {
            return Err("agent session platform session id must be non-empty".to_owned());
        }
        Ok(())
    }
}

pub const fn choice_plane_transition(
    namespace_state: DurableNamespaceState,
) -> (NamespaceAction, ChoicePlaneDecision, CodexHostAction) {
    match namespace_state {
        DurableNamespaceState::Absent => (
            NamespaceAction::Create,
            ChoicePlaneDecision::CreateNew,
            CodexHostAction::SpawnAgent,
        ),
        DurableNamespaceState::Present => (
            NamespaceAction::Resume,
            ChoicePlaneDecision::ContinueExisting,
            CodexHostAction::FollowupTask,
        ),
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AgentSessionNamespaceProjection {
    pub schema_id: String,
    pub schema_version: String,
    pub project_id: String,
    pub workspace_id: String,
    pub canonical_workspace_root: String,
    pub session_identity: PlatformSessionIdentity,
    pub root_namespace_id: String,
    pub agent_name: String,
    pub generation: u64,
    pub namespace_state: DurableNamespaceState,
    pub namespace_action: NamespaceAction,
    pub host_liveness: HostChildLiveness,
    pub decision: ChoicePlaneDecision,
    pub host_action: CodexHostAction,
}

impl AgentSessionNamespaceProjection {
    pub fn new(
        project_id: impl Into<String>,
        workspace_id: impl Into<String>,
        canonical_workspace_root: impl Into<String>,
        session_identity: PlatformSessionIdentity,
        root_namespace_id: impl Into<String>,
        agent_name: impl Into<String>,
        generation: u64,
        namespace_state: DurableNamespaceState,
        host_liveness: HostChildLiveness,
    ) -> Self {
        let (namespace_action, decision, host_action) = choice_plane_transition(namespace_state);
        Self {
            schema_id: AGENT_SESSION_NAMESPACE_PROJECTION_SCHEMA_ID.to_owned(),
            schema_version: AGENT_SESSION_NAMESPACE_PROJECTION_SCHEMA_VERSION.to_owned(),
            project_id: project_id.into(),
            workspace_id: workspace_id.into(),
            canonical_workspace_root: canonical_workspace_root.into(),
            session_identity,
            root_namespace_id: root_namespace_id.into(),
            agent_name: agent_name.into(),
            generation,
            namespace_state,
            namespace_action,
            host_liveness,
            decision,
            host_action,
        }
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.schema_id != AGENT_SESSION_NAMESPACE_PROJECTION_SCHEMA_ID {
            return Err("agent session namespace projection schemaId mismatch".to_owned());
        }
        if self.schema_version != AGENT_SESSION_NAMESPACE_PROJECTION_SCHEMA_VERSION {
            return Err("agent session namespace projection schemaVersion mismatch".to_owned());
        }
        if self.root_namespace_id.trim().is_empty() {
            return Err("agent session root namespace id must be non-empty".to_owned());
        }
        if self.project_id.trim().is_empty() {
            return Err("agent session project id must be non-empty".to_owned());
        }
        if self.workspace_id.trim().is_empty() {
            return Err("agent session workspace id must be non-empty".to_owned());
        }
        if self.canonical_workspace_root.trim().is_empty() {
            return Err("agent session canonical workspace root must be non-empty".to_owned());
        }
        self.session_identity.validate()?;
        if !is_registered_agent_name(&self.agent_name) {
            return Err("agent session agent name must be a registered @name".to_owned());
        }
        if (self.namespace_action, self.decision, self.host_action)
            != choice_plane_transition(self.namespace_state)
        {
            return Err(
                "agent session namespace transition does not match durable namespace state"
                    .to_owned(),
            );
        }
        Ok(())
    }

    pub fn namespace_key(&self) -> AgentSessionNamespaceKey {
        AgentSessionNamespaceKey {
            project_id: self.project_id.clone(),
            workspace_id: self.workspace_id.clone(),
            canonical_workspace_root: self.canonical_workspace_root.clone(),
            platform: self.session_identity.platform.clone(),
            platform_session_id: self.session_identity.platform_session_id.clone(),
            root_namespace_id: self.root_namespace_id.clone(),
            agent_name: self.agent_name.clone(),
        }
    }
}

fn is_registered_agent_name(agent_name: &str) -> bool {
    let Some(name) = agent_name.strip_prefix('@') else {
        return false;
    };
    let mut chars = name.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    first.is_ascii_lowercase()
        && chars.all(|character| {
            character.is_ascii_lowercase()
                || character.is_ascii_digit()
                || matches!(character, '_' | '-')
        })
}
