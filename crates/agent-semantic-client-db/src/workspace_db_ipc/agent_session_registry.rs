//! Typed Runtime IPC contract for the single DB-owned agent-session registry authority.

use serde::{Deserialize, Serialize};

use crate::SessionControlPlaneSnapshot;
use crate::agent_session_registry::{
    AgentSessionCommandDigest, AgentSessionDeliveryTargetId, AgentSessionDispatchIdentity,
    AgentSessionEvidenceRef, AgentSessionId, AgentSessionMessageTargetId, AgentSessionMetadataJson,
    AgentSessionModelEvidenceRef, AgentSessionModelId, AgentSessionModelObservationSourceId,
    AgentSessionProjectId, AgentSessionResidentName, AgentSessionRole, AgentSessionRootSessionId,
    AgentSessionStatus,
};

use super::protocol::WorkspaceDbIpcSession;
use super::{WorkspaceDbIpcOperation, WorkspaceDbIpcResult};

/// Closed operation set accepted by the Runtime-owned agent-session registry.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "kebab-case",
    rename_all_fields = "camelCase"
)]
pub enum AgentSessionRegistryIpcOperation {
    RegisterControlPlaneAgent {
        registration: SessionControlPlaneAgentRegistration,
    },
    AdmitControlPlaneDelegation {
        proposal: SessionControlPlaneDelegationProposal,
    },
    ReadControlPlaneSnapshot {
        project_id: AgentSessionProjectId,
        root_session_id: AgentSessionRootSessionId,
    },
    RecordHostLifecycleEvent {
        event: AgentHostLifecycleEventIpc,
    },
    RecordHostExecutionObservation {
        observation: AgentHostExecutionObservationIpc,
    },
    RecordHostNonMatch {
        observation: AgentHostNonMatchIpc,
    },
    Register {
        request: AgentSessionRegisterIpcRequest,
    },
    Query {
        project_id: AgentSessionProjectId,
        root_session_id: Option<AgentSessionRootSessionId>,
        name: Option<AgentSessionResidentName>,
    },
    SessionById {
        project_id: AgentSessionProjectId,
        session_id: AgentSessionId,
    },
    SessionByName {
        project_id: AgentSessionProjectId,
        root_session_id: AgentSessionRootSessionId,
        name: AgentSessionResidentName,
    },
    UpdateStatus {
        project_id: AgentSessionProjectId,
        session_id: AgentSessionId,
        status: AgentSessionStatus,
        now: i64,
    },
    SetArchivedStatus {
        project_id: AgentSessionProjectId,
        session_id: AgentSessionId,
        archived: bool,
        now: i64,
    },
    SessionIsRetired {
        project_id: AgentSessionProjectId,
        session_id: AgentSessionId,
    },
    RefreshExpired,
    SessionByIdAnyProject {
        session_id: AgentSessionId,
    },
    ProjectIdForRootSessionId {
        root_session_id: AgentSessionRootSessionId,
    },
    ClaimDispatch {
        project_id: AgentSessionProjectId,
        root_session_id: AgentSessionRootSessionId,
        child_session_id: AgentSessionId,
        name: AgentSessionResidentName,
        dispatch_identity: AgentSessionDispatchIdentity,
        command_digest: AgentSessionCommandDigest,
        delivery_target_override: Option<AgentSessionDeliveryTargetId>,
        now: i64,
    },
    DispatchLease {
        project_id: AgentSessionProjectId,
        root_session_id: AgentSessionRootSessionId,
        name: AgentSessionResidentName,
        dispatch_identity: AgentSessionDispatchIdentity,
    },
    CompleteDispatch {
        project_id: AgentSessionProjectId,
        root_session_id: AgentSessionRootSessionId,
        name: AgentSessionResidentName,
        dispatch_identity: AgentSessionDispatchIdentity,
        command_digest: AgentSessionCommandDigest,
        evidence_ref: AgentSessionEvidenceRef,
        now: i64,
    },
}

/// Host lifecycle transition observed from the native client host.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AgentHostLifecycleEventKind {
    Started,
    Resumed,
    Stopped,
    Achieved,
}

/// Sandbox authority reported by the native host for one agent session.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AgentHostSandboxMode {
    ReadOnly,
    WorkspaceWrite,
    DangerFullAccess,
}

impl AgentHostSandboxMode {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ReadOnly => "read-only",
            Self::WorkspaceWrite => "workspace-write",
            Self::DangerFullAccess => "danger-full-access",
        }
    }
}

impl std::fmt::Display for AgentHostSandboxMode {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl TryFrom<&str> for AgentHostSandboxMode {
    type Error = String;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value {
            "read-only" => Ok(Self::ReadOnly),
            "workspace-write" => Ok(Self::WorkspaceWrite),
            "danger-full-access" => Ok(Self::DangerFullAccess),
            _ => Err(format!("unsupported host sandbox mode: {value}")),
        }
    }
}

macro_rules! agent_host_identifier {
    ($(#[$meta:meta])* $name:ident) => {
        $(#[$meta])*
        #[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
        #[serde(transparent)]
        pub struct $name(String);

        impl $name {
            #[must_use]
            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl From<String> for $name {
            fn from(value: String) -> Self {
                Self(value)
            }
        }

        impl From<&str> for $name {
            fn from(value: &str) -> Self {
                Self(value.to_owned())
            }
        }

        impl std::ops::Deref for $name {
            type Target = str;

            fn deref(&self) -> &Self::Target {
                self.as_str()
            }
        }

        impl std::fmt::Display for $name {
            fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                formatter.write_str(self.as_str())
            }
        }
    };
}

agent_host_identifier!(
    /// Stable identity of one native host lifecycle event.
    AgentHostEventId
);
agent_host_identifier!(
    /// Durable namespace identity bound to one resident child generation.
    AgentHostNamespaceId
);
agent_host_identifier!(
    /// Registered route identity used by the host lifecycle projection.
    AgentHostRouteKey
);
agent_host_identifier!(
    /// Profile identity admitted for one host agent.
    AgentHostProfileId
);
agent_host_identifier!(
    /// Native transcript path retained only as optional lifecycle evidence.
    AgentHostTranscriptPath
);

/// Typed host lifecycle evidence admitted into the Runtime registry.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AgentHostLifecycleEventIpc {
    pub host_event_id: AgentHostEventId,
    pub host_event_sequence: u64,
    pub namespace_id: AgentHostNamespaceId,
    pub kind: AgentHostLifecycleEventKind,
    pub platform: String,
    pub project_id: AgentSessionProjectId,
    pub root_session_id: AgentSessionRootSessionId,
    pub parent_session_id: AgentSessionId,
    pub child_session_id: AgentSessionId,
    pub host_task_name: String,
    pub platform_host_agent_name: String,
    pub route_key: AgentHostRouteKey,
    pub profile_id: AgentHostProfileId,
    pub role: String,
    pub model: String,
    pub model_digest: String,
    pub profile_digest: String,
    pub sandbox_mode: AgentHostSandboxMode,
    pub session_lifetime: String,
    pub payload_digest: String,
    pub transcript_path: Option<AgentHostTranscriptPath>,
    pub observed_at: i64,
}

/// Native host execution observation bound to one child session.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AgentHostExecutionObservationIpc {
    pub observation_id: String,
    pub project_id: AgentSessionProjectId,
    pub root_session_id: AgentSessionRootSessionId,
    pub child_session_id: AgentSessionId,
    pub platform_host_agent_name: String,
    pub transcript_path: String,
    pub observed_at: i64,
}

/// Host event that could not be matched to a registered session.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AgentHostNonMatchIpc {
    pub kind: AgentHostLifecycleEventKind,
    pub project_id: AgentSessionProjectId,
    pub root_session_id: AgentSessionRootSessionId,
    pub child_session_id: AgentSessionId,
    pub host_task_name: String,
    pub payload_digest: String,
    pub observed_at: i64,
}

/// IPC projection of one typed session registration request.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentSessionRegisterIpcRequest {
    pub project_id: AgentSessionProjectId,
    pub root_session_id: AgentSessionRootSessionId,
    pub session_id: AgentSessionId,
    pub message_target_id: Option<AgentSessionMessageTargetId>,
    pub parent_session_id: Option<AgentSessionId>,
    pub name: AgentSessionResidentName,
    pub role: AgentSessionRole,
    pub model_observation: Option<AgentSessionModelObservationIpc>,
    pub status: crate::agent_session_registry::AgentSessionStatus,
    pub expires_at: Option<i64>,
    pub metadata_json: AgentSessionMetadataJson,
    pub now: i64,
}

/// Typed model observation carried with a session registration.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentSessionModelObservationIpc {
    pub model: AgentSessionModelId,
    pub source: AgentSessionModelObservationSourceId,
    pub observed_at: i64,
    pub evidence_ref: Option<AgentSessionModelEvidenceRef>,
}

/// Closed result set returned by the Runtime registry authority.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "kebab-case",
    rename_all_fields = "camelCase"
)]
pub enum AgentSessionRegistryIpcResult {
    ControlPlaneAgentRegistered,
    ControlPlaneDelegationAdmitted {
        receipt: SessionControlPlaneTransactionReceipt,
    },
    ControlPlaneSnapshot {
        snapshot: SessionControlPlaneSnapshot,
    },
    Session {
        session: Option<crate::AgentSessionRecord>,
    },
    Sessions {
        sessions: Vec<crate::AgentSessionRecord>,
    },
    Registered {
        session: crate::AgentSessionRecord,
    },
    Changed {
        changed: bool,
    },
    Refreshed,
    ProjectId {
        project_id: Option<AgentSessionProjectId>,
    },
    DispatchClaimed {
        result: crate::agent_session_registry::AgentSessionDispatchClaimResult,
    },
    DispatchLease {
        lease: Option<crate::agent_session_registry::AgentSessionDispatchLeaseRecord>,
    },
    DispatchCompleted {
        lease: crate::agent_session_registry::AgentSessionDispatchLeaseRecord,
    },
}

async fn bounded_agent_session_registry_call<T>(
    timeout: std::time::Duration,
    future: impl std::future::Future<Output = Result<T, String>>,
) -> Result<T, String> {
    tokio::time::timeout(timeout, future).await.map_err(|_| {
        format!(
            "Runtime Server agent-session registry IPC timed out after {}ms",
            timeout.as_millis()
        )
    })?
}

impl WorkspaceDbIpcSession {
    pub async fn call_agent_session_registry(
        &self,
        operation: AgentSessionRegistryIpcOperation,
    ) -> Result<AgentSessionRegistryIpcResult, String> {
        let result = bounded_agent_session_registry_call(
            std::time::Duration::from_secs(5),
            self.call_operation(WorkspaceDbIpcOperation::AgentSessionRegistry {
                project_root: self.runtime_project_root()?.display().to_string(),
                operation,
            }),
        )
        .await?;
        match result {
            WorkspaceDbIpcResult::AgentSessionRegistry { result } => Ok(result),
            _ => Err(
                "Runtime Server returned an unexpected agent-session registry result".to_owned(),
            ),
        }
    }
}

#[cfg(test)]
#[path = "../../tests/unit/workspace_db_ipc_agent_session_registry.rs"]
mod tests;
use crate::{
    SessionControlPlaneAgentRegistration, SessionControlPlaneDelegationProposal,
    SessionControlPlaneTransactionReceipt,
};
