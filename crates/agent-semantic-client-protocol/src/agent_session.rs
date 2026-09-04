//! Typed wire contracts for registering a configured Codex child Agent session.

use serde::Deserialize;
use serde::Serialize;

use crate::AgentChildThreadId;
use crate::AgentName;
use crate::AgentParentThreadId;
use crate::AgentPath;
use crate::AgentRootSessionId;
use crate::AgentRouteKey;
use crate::ClientProjectId;
use crate::ClientSchemaId;

/// Runtime method that registers the current child thread beneath its parent thread.
pub const AGENT_SESSION_REGISTER_METHOD: &str = "asp.session.register-child";
/// Schema identity for a child-session registration request.
pub const AGENT_SESSION_REGISTER_REQUEST_SCHEMA_ID: &str =
    "agent.semantic-protocols.codex-child-session-registration-request";
/// Schema identity for a successful child-session registration receipt.
pub const AGENT_SESSION_REGISTER_RESPONSE_SCHEMA_ID: &str =
    "agent.semantic-protocols.codex-child-session-registration-receipt";

/// Terminal state of a committed child-session registration.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum AgentSessionRegisterState {
    /// The Runtime registry committed the parent-child binding.
    Registered,
}

/// Host platform that owns the registered child thread.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum AgentSessionPlatform {
    /// Codex owns the thread and collaboration path.
    Codex,
}

/// Runtime component that owns the durable registration fact.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum AgentSessionRegistryOwner {
    /// The Runtime Server AgentSession Registry owns the binding.
    RuntimeServerAgentSessionRegistry,
}

/// Transport used to commit a child-session registration.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum AgentSessionTransport {
    /// Registration was committed through a Runtime gRPC client frame.
    GrpcClientFrame,
}

/// Parent-authored identity binding submitted to the Runtime-owned session registry.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AgentSessionRegisterRequest {
    /// Request schema identity.
    pub schema_id: ClientSchemaId,
    /// Request schema version.
    pub schema_version: u64,
    /// Root Codex session shared by the parent and child threads.
    pub root_session_id: AgentRootSessionId,
    /// Persistent parent thread identity.
    pub parent_thread_id: AgentParentThreadId,
    /// Persistent child thread identity read by the child process.
    pub child_thread_id: AgentChildThreadId,
    /// Configured Agent name without its canonical path prefix.
    pub agent_name: AgentName,
    /// Canonical collaboration path for the configured Agent.
    pub agent_path: AgentPath,
    /// Config-resolved route key bound to the Agent.
    pub route_key: AgentRouteKey,
}

/// Durable receipt returned after the Runtime registry commits the child binding.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AgentSessionRegisterReceipt {
    /// Receipt schema identity.
    pub schema_id: ClientSchemaId,
    /// Receipt schema version.
    pub schema_version: u64,
    /// Terminal registration state.
    pub state: AgentSessionRegisterState,
    /// Host platform that owns the child thread.
    pub platform: AgentSessionPlatform,
    /// Workspace project identity used by the registry.
    pub project_id: ClientProjectId,
    /// Root Codex session shared by the parent and child threads.
    pub root_session_id: AgentRootSessionId,
    /// Persistent parent thread identity.
    pub parent_thread_id: AgentParentThreadId,
    /// Persistent child thread identity.
    pub child_thread_id: AgentChildThreadId,
    /// Configured Agent name.
    pub agent_name: AgentName,
    /// Canonical collaboration path for the Agent.
    pub agent_path: AgentPath,
    /// Config-resolved route key bound to the Agent.
    pub route_key: AgentRouteKey,
    /// Nonzero physical registry generation of this binding.
    pub physical_generation: u64,
    /// Runtime component that owns the durable registration.
    pub registry_owner: AgentSessionRegistryOwner,
    /// Transport used to commit the registration.
    pub transport: AgentSessionTransport,
}

impl AgentSessionRegisterRequest {
    /// Validates schema identity and the root, parent, child, Agent, and route binding.
    pub fn validate(&self) -> Result<(), String> {
        if self.schema_id.as_str() != AGENT_SESSION_REGISTER_REQUEST_SCHEMA_ID
            || self.schema_version != 1
        {
            return Err("child registration request schema identity is invalid".to_owned());
        }
        for (field, value) in [
            ("rootSessionId", self.root_session_id.as_str()),
            ("parentThreadId", self.parent_thread_id.as_str()),
            ("childThreadId", self.child_thread_id.as_str()),
            ("agentName", self.agent_name.as_str()),
            ("agentPath", self.agent_path.as_str()),
            ("routeKey", self.route_key.as_str()),
        ] {
            if value.trim().is_empty() {
                return Err(format!("child registration {field} must be non-empty"));
            }
        }
        if self.parent_thread_id.as_str() == self.child_thread_id.as_str() {
            return Err(
                "child registration requires distinct parent and child thread ids".to_owned(),
            );
        }
        if self.agent_path.as_str() != format!("/root/{}", self.agent_name.as_str()) {
            return Err("child registration agentPath does not match agentName".to_owned());
        }
        Ok(())
    }
}

impl AgentSessionRegisterReceipt {
    /// Validates a current receipt and reconstructs its request-side identity invariant.
    pub fn validate(&self) -> Result<(), String> {
        if self.schema_id.as_str() != AGENT_SESSION_REGISTER_RESPONSE_SCHEMA_ID
            || self.schema_version != 1
            || self.state != AgentSessionRegisterState::Registered
            || self.platform != AgentSessionPlatform::Codex
            || self.registry_owner != AgentSessionRegistryOwner::RuntimeServerAgentSessionRegistry
            || self.transport != AgentSessionTransport::GrpcClientFrame
            || self.physical_generation == 0
        {
            return Err("child registration receipt is not current".to_owned());
        }
        AgentSessionRegisterRequest {
            schema_id: ClientSchemaId::new(AGENT_SESSION_REGISTER_REQUEST_SCHEMA_ID)?,
            schema_version: 1,
            root_session_id: self.root_session_id.clone(),
            parent_thread_id: self.parent_thread_id.clone(),
            child_thread_id: self.child_thread_id.clone(),
            agent_name: self.agent_name.clone(),
            agent_path: self.agent_path.clone(),
            route_key: self.route_key.clone(),
        }
        .validate()?;
        if self.project_id.as_str().trim().is_empty() {
            return Err("child registration receipt projectId must be non-empty".to_owned());
        }
        Ok(())
    }
}

#[cfg(test)]
#[path = "../tests/unit/agent_session.rs"]
mod tests;
