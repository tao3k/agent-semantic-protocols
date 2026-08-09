use serde::{Deserialize, Serialize};

use crate::SessionControlPlaneSnapshot;

use super::protocol::WorkspaceDbIpcSession;
use super::{WorkspaceDbIpcOperation, WorkspaceDbIpcResult};

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
        project_id: String,
        root_session_id: String,
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
        project_id: String,
        root_session_id: Option<String>,
        name: Option<String>,
    },
    SessionById {
        project_id: String,
        session_id: String,
    },
    SessionByName {
        project_id: String,
        root_session_id: String,
        name: String,
    },
    UpdateStatus {
        project_id: String,
        session_id: String,
        status: String,
        now: i64,
    },
    SetArchivedStatus {
        project_id: String,
        session_id: String,
        archived: bool,
        now: i64,
    },
    SessionIsRetired {
        project_id: String,
        session_id: String,
    },
    RefreshExpired,
    SessionByIdAnyProject {
        session_id: String,
    },
    ProjectIdForRootSessionId {
        root_session_id: String,
    },
    ClaimDispatch {
        project_id: String,
        root_session_id: String,
        name: String,
        dispatch_identity: String,
        command_digest: String,
        delivery_target_override: Option<String>,
        now: i64,
    },
    DispatchLease {
        project_id: String,
        root_session_id: String,
        name: String,
        dispatch_identity: String,
    },
    CompleteDispatch {
        project_id: String,
        root_session_id: String,
        name: String,
        dispatch_identity: String,
        command_digest: String,
        evidence_ref: String,
        now: i64,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AgentHostLifecycleEventKind {
    Started,
    Resumed,
    Stopped,
    Achieved,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AgentHostLifecycleEventIpc {
    pub host_event_id: String,
    pub host_event_sequence: u64,
    pub namespace_id: String,
    pub kind: AgentHostLifecycleEventKind,
    pub platform: String,
    pub project_id: String,
    pub root_session_id: String,
    pub parent_session_id: String,
    pub child_session_id: String,
    pub host_task_name: String,
    pub platform_host_agent_name: String,
    pub route_key: String,
    pub profile_id: String,
    pub role: String,
    pub model: String,
    pub model_digest: String,
    pub profile_digest: String,
    pub sandbox_mode: String,
    pub session_lifetime: String,
    pub payload_digest: String,
    pub transcript_path: Option<String>,
    pub observed_at: i64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AgentHostExecutionObservationIpc {
    pub observation_id: String,
    pub project_id: String,
    pub root_session_id: String,
    pub child_session_id: String,
    pub platform_host_agent_name: String,
    pub transcript_path: String,
    pub observed_at: i64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AgentHostNonMatchIpc {
    pub kind: AgentHostLifecycleEventKind,
    pub project_id: String,
    pub root_session_id: String,
    pub child_session_id: String,
    pub host_task_name: String,
    pub payload_digest: String,
    pub observed_at: i64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentSessionRegisterIpcRequest {
    pub project_id: String,
    pub root_session_id: String,
    pub session_id: String,
    pub message_target_id: Option<String>,
    pub parent_session_id: Option<String>,
    pub name: String,
    pub role: String,
    pub model_observation: Option<AgentSessionModelObservationIpc>,
    pub status: String,
    pub expires_at: Option<i64>,
    pub metadata_json: String,
    pub now: i64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentSessionModelObservationIpc {
    pub model: String,
    pub source: String,
    pub observed_at: i64,
    pub evidence_ref: Option<String>,
}

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
        project_id: Option<String>,
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
