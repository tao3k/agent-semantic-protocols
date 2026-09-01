use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use agent_semantic_artifacts::runtime_artifact_catalog::RuntimeBinaryIdentity;
use serde::{Deserialize, Serialize};

pub(super) const SCHEMA_VERSION: &str = "1";
pub(super) const ENDPOINT_SCHEMA_ID: &str = "agent.semantic-protocols.runtime-server-endpoint";
pub(super) const REQUEST_SCHEMA_ID: &str =
    "agent.semantic-protocols.runtime-server-control-request";
pub(super) const RECEIPT_SCHEMA_ID: &str =
    "agent.semantic-protocols.runtime-server-control-receipt";
const STATUS_SNAPSHOT_SCHEMA_ID: &str = "agent.semantic-protocols.runtime-server-status-snapshot";

static RUNTIME_SERVER_TRANSPORT_CONTRACT_DIGEST: OnceLock<String> = OnceLock::new();
const RUNTIME_SERVER_TRANSPORT_CONTRACT_DOMAIN: &[u8] =
    b"agent.semantic-protocols.runtime-server-transport";
const RUNTIME_SERVER_CONTROL_CONTRACT: &[u8] =
    include_bytes!("../../../../schemas/runtime-server-control.v1.schema.json");
const WORKSPACE_DB_OWNER_IPC_CONTRACT: &[u8] =
    include_bytes!("../../../../schemas/workspace-db-owner-ipc.v1.schema.json");
const RUNTIME_PERFORMANCE_OBSERVATION_CONTRACT: &[u8] =
    include_bytes!("../../../../schemas/runtime-server-performance-observation.v1.schema.json");
const RUNTIME_PERFORMANCE_INGRESS_RECEIPT_CONTRACT: &[u8] =
    include_bytes!("../../../../schemas/runtime-server-performance-ingress-receipt.v1.schema.json");
const PROVIDER_REGISTER_REQUEST_CONTRACT: &[u8] =
    include_bytes!("../../../../schemas/provider-register-request.schema.json");
const PROVIDER_REGISTER_RESPONSE_CONTRACT: &[u8] =
    include_bytes!("../../../../schemas/provider-register-response.schema.json");
const ASP_CLIENT_FRAME_CONTRACT: &[u8] =
    include_bytes!("../../../../schemas/asp-client-frame.schema.json");

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeServerEndpoint {
    pub schema_id: String,
    pub schema_version: String,
    pub binary_content_digest: String,
    pub runtime_generation_digest: String,
    pub schema_digest: String,
    pub transport_contract_digest: String,
    pub owner_epoch: u64,
    #[serde(default)]
    pub owner_process_id: u32,
    pub runtime_artifact_path: String,
    pub runtime_binary_identity: RuntimeBinaryIdentity,
    pub monitor_capability: bool,
    pub observed_runtime_binary_identity: RuntimeBinaryIdentity,
    pub artifact_mode: String,
    pub artifact_catalog_digest: String,
    pub binding_token: String,
    pub socket_path: String,
    pub data_plane_socket_path: String,
    pub provider_plane_socket_path: String,
    pub workspace_store_path: String,
    pub status_memory_path: String,
}

/// Stable owner envelope used only by lifecycle coordination.
///
/// Service fields may evolve while schema version 1 is under development, but
/// lifecycle handoff must still be able to prove which process owns an
/// otherwise undecodable endpoint. Keep this projection deliberately smaller
/// than [`RuntimeServerEndpoint`]: it is authority for verified retirement,
/// never authority for serving requests.
#[derive(Clone, Debug, Eq, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeServerEndpointOwnerBinding {
    pub schema_id: String,
    pub schema_version: String,
    pub owner_epoch: u64,
    #[serde(default)]
    pub owner_process_id: u32,
    pub runtime_artifact_path: String,
    pub binding_token: String,
}

impl RuntimeServerEndpointOwnerBinding {
    pub fn validate(&self) -> Result<(), String> {
        if self.schema_id != ENDPOINT_SCHEMA_ID || self.schema_version != SCHEMA_VERSION {
            return Err("Runtime Server endpoint owner schema identity mismatch".to_owned());
        }
        if self.owner_epoch == 0
            || self.owner_process_id == 0
            || self.binding_token.is_empty()
            || !Path::new(&self.runtime_artifact_path).is_absolute()
        {
            return Err("Runtime Server endpoint owner binding is incomplete".to_owned());
        }
        Ok(())
    }
}

impl RuntimeServerEndpoint {
    pub fn validate(&self) -> Result<(), String> {
        self.validate_supervisor_control()?;
        let expected = runtime_server_transport_contract_digest_ref();
        if self.transport_contract_digest != expected {
            return Err(format!(
                "Runtime Server transport contract mismatch: expected={expected} actual={}",
                self.transport_contract_digest
            ));
        }
        Ok(())
    }

    pub fn validate_supervisor_control(&self) -> Result<(), String> {
        if self.schema_id != ENDPOINT_SCHEMA_ID || self.schema_version != SCHEMA_VERSION {
            return Err("Runtime Server endpoint schema identity mismatch".to_owned());
        }
        if self.owner_epoch == 0
            || self.transport_contract_digest.is_empty()
            || self.runtime_artifact_path.is_empty()
            || self.binary_content_digest.is_empty()
            || self.runtime_generation_digest.is_empty()
            || self.schema_digest.is_empty()
            || !matches!(self.artifact_mode.as_str(), "dev" | "release")
            || !is_blake3_digest(&self.artifact_catalog_digest)
            || self.binding_token.is_empty()
            || self.socket_path.is_empty()
            || self.data_plane_socket_path.is_empty()
            || self.provider_plane_socket_path.is_empty()
            || self.workspace_store_path.is_empty()
            || self.status_memory_path.is_empty()
        {
            return Err("Runtime Server endpoint is incomplete".to_owned());
        }
        if self.binary_content_digest != self.runtime_binary_identity.content_digest().as_str() {
            return Err(
                "reasonKind=runtime-server-endpoint-identity-incomplete binaryContentDigest does not match canonical Runtime binary identity"
                    .to_owned(),
            );
        }
        if !is_blake3_digest(&self.runtime_generation_digest) {
            return Err(format!(
                "reasonKind=runtime-server-endpoint-identity-incomplete field=runtimeGenerationDigest value={} Runtime generation digest is invalid",
                self.runtime_generation_digest
            ));
        }
        if !is_blake3_digest(&self.schema_digest) {
            return Err(format!(
                "reasonKind=runtime-server-endpoint-identity-incomplete field=schemaDigest value={} Runtime schema digest is invalid",
                self.schema_digest
            ));
        }
        if !Path::new(&self.socket_path).is_absolute() {
            return Err("Runtime Server socket path must be absolute".to_owned());
        }
        if !Path::new(&self.data_plane_socket_path).is_absolute() {
            return Err("Runtime Server data-plane socket path must be absolute".to_owned());
        }
        if !Path::new(&self.workspace_store_path).is_absolute() {
            return Err("Runtime Server workspace store path must be absolute".to_owned());
        }
        if !Path::new(&self.status_memory_path).is_absolute() {
            return Err("Runtime Server status memory path must be absolute".to_owned());
        }
        Ok(())
    }

    /// Prove that every service published by this endpoint generation still
    /// has a live listener.
    ///
    /// Status memory is only a cache owned by the generation; it is not
    /// sufficient evidence that the control, data, provider, and public client
    /// planes still agree on that generation.  All loopback connects are
    /// started together so a healthy receipt cannot be assembled from a stale
    /// mmap snapshot plus a newer or partially drained listener set.
    pub async fn validate_service_reachability(&self) -> Result<(), String> {
        self.validate()?;
        let control = async {
            tokio::net::UnixStream::connect(&self.socket_path)
                .await
                .map(|_| ())
                .map_err(|error| {
                    format!(
                        "Runtime Server control listener is unreachable for endpoint generation {}: {error}",
                        self.owner_epoch
                    )
                })
        };
        let data = async {
            tokio::net::UnixStream::connect(&self.data_plane_socket_path)
                .await
                .map(|_| ())
                .map_err(|error| {
                    format!(
                        "Runtime Server data listener is unreachable for endpoint generation {}: {error}",
                        self.owner_epoch
                    )
                })
        };
        let provider = async {
            tokio::net::UnixStream::connect(&self.provider_plane_socket_path)
                .await
                .map(|_| ())
                .map_err(|error| {
                    format!(
                        "Runtime Server provider listener is unreachable for endpoint generation {}: {error}",
                        self.owner_epoch
                    )
                })
        };
        tokio::try_join!(control, data, provider)?;
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum RuntimeServerOperation {
    Status,
    EnsureWorkspace,
    Restart,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RuntimeServerControlRequest {
    pub schema_id: String,
    pub schema_version: String,
    pub operation: RuntimeServerOperation,
    pub expected_runtime_binary_identity: RuntimeBinaryIdentity,
    pub request_id: String,
    pub transport_contract_digest: String,
    pub owner_epoch: u64,
    pub binding_token: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project_root: Option<String>,
}

impl RuntimeServerControlRequest {
    pub fn validate_for_endpoint(&self, endpoint: &RuntimeServerEndpoint) -> Result<(), String> {
        self.validate_supervisor_binding(endpoint)?;
        if self.operation == RuntimeServerOperation::Status
            && self.transport_contract_digest != endpoint.transport_contract_digest
        {
            return Err(format!(
                "Runtime Server transport contract mismatch: expected={} actual={}",
                self.transport_contract_digest, endpoint.transport_contract_digest
            ));
        }
        Ok(())
    }

    fn validate_supervisor_binding(&self, endpoint: &RuntimeServerEndpoint) -> Result<(), String> {
        if self.schema_id != REQUEST_SCHEMA_ID || self.schema_version != SCHEMA_VERSION {
            return Err("Runtime Server control request schema identity mismatch".to_owned());
        }
        if self.request_id.is_empty()
            || self.transport_contract_digest.is_empty()
            || self.owner_epoch != endpoint.owner_epoch
            || self.binding_token != endpoint.binding_token
        {
            return Err("Runtime Server control request binding mismatch".to_owned());
        }
        Ok(())
    }

    pub fn requires_restart(&self, endpoint: &RuntimeServerEndpoint) -> Result<bool, String> {
        self.validate_supervisor_binding(endpoint)?;
        match self.operation {
            RuntimeServerOperation::Status => {
                self.validate_for_endpoint(endpoint)?;
                Ok(false)
            }
            RuntimeServerOperation::EnsureWorkspace => {
                self.validate_for_endpoint(endpoint)?;
                if self
                    .project_root
                    .as_deref()
                    .is_none_or(|project_root| project_root.trim().is_empty())
                {
                    return Err(
                        "Runtime Server ensure-workspace requires a project root".to_owned()
                    );
                }
                Ok(false)
            }
            RuntimeServerOperation::Restart => Ok(true),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum RuntimeServerState {
    Healthy,
    Starting,
    Draining,
    Degraded,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeServerStatusSnapshot {
    pub schema_id: String,
    pub schema_version: String,
    pub generation: u64,
    pub state: RuntimeServerState,
    pub runtime_binary_identity: RuntimeBinaryIdentity,
    pub artifact_mode: String,
    pub artifact_catalog_digest: String,
    pub transport_contract_digest: String,
    pub workspace_entry_count: usize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub asp_python_graphs: Option<AspPythonGraphsStatus>,
    #[serde(default)]
    pub agent_sessions: Vec<RuntimeServerAgentSessionStatus>,
    pub owner_epoch: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum RuntimeServerAgentSessionLifecycleState {
    Routable,
    Stopped,
    Achieved,
    Invalid,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeServerAgentSessionStatus {
    pub workspace_identity: String,
    pub project_id: String,
    pub root_session_id: String,
    pub session_id: String,
    pub name: String,
    pub physical_generation: u64,
    pub lifecycle_state: RuntimeServerAgentSessionLifecycleState,
    #[serde(default)]
    pub host_binding: Option<serde_json::Value>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentSessionControlPlaneState {
    pub project_id: Option<String>,
    pub root_session_id: Option<String>,
    pub name: String,
    pub state: String,
    pub generation: u64,
    pub reason_kind: Option<String>,
    pub host_binding: Option<serde_json::Value>,
}

impl RuntimeServerStatusSnapshot {
    pub(crate) fn new(
        generation: u64,
        state: RuntimeServerState,
        endpoint: &RuntimeServerEndpoint,
        workspace_entry_count: usize,
    ) -> Self {
        Self {
            schema_id: STATUS_SNAPSHOT_SCHEMA_ID.to_owned(),
            schema_version: SCHEMA_VERSION.to_owned(),
            generation,
            state,
            runtime_binary_identity: endpoint.runtime_binary_identity.clone(),
            artifact_mode: endpoint.artifact_mode.clone(),
            artifact_catalog_digest: endpoint.artifact_catalog_digest.clone(),
            transport_contract_digest: endpoint.transport_contract_digest.clone(),
            workspace_entry_count,
            asp_python_graphs: None,
            agent_sessions: Vec::new(),
            owner_epoch: endpoint.owner_epoch,
        }
    }

    pub(crate) fn validate(&self, observed_generation: u64) -> Result<(), String> {
        if self.schema_id != STATUS_SNAPSHOT_SCHEMA_ID
            || self.schema_version != SCHEMA_VERSION
            || self.generation != observed_generation
            || self.generation == 0
            || self.generation % 2 != 0
        {
            return Err("Runtime Server status memory identity mismatch".to_owned());
        }
        Ok(())
    }

    pub(crate) fn receipt(
        &self,
        request_id: String,
        endpoint: &RuntimeServerEndpoint,
    ) -> Result<RuntimeServerControlReceipt, String> {
        if self.owner_epoch != endpoint.owner_epoch
            || self.runtime_binary_identity != endpoint.runtime_binary_identity
            || self.artifact_mode != endpoint.artifact_mode
            || self.artifact_catalog_digest != endpoint.artifact_catalog_digest
            || self.transport_contract_digest != endpoint.transport_contract_digest
        {
            return Err("Runtime Server status memory binding mismatch".to_owned());
        }
        Ok(RuntimeServerControlReceipt {
            schema_id: RECEIPT_SCHEMA_ID.to_owned(),
            schema_version: SCHEMA_VERSION.to_owned(),
            request_id,
            state: self.state,
            runtime_binary_identity: self.runtime_binary_identity.clone(),
            artifact_mode: self.artifact_mode.clone(),
            artifact_catalog_digest: self.artifact_catalog_digest.clone(),
            transport_contract_digest: self.transport_contract_digest.clone(),
            workspace_entry_count: self.workspace_entry_count,
            workspace_generation: None,
            asp_python_graphs: self.asp_python_graphs.clone(),
            resident_transaction: None,
            reason: None,
        })
    }

    pub(crate) fn cached_health_receipt(&self, request_id: String) -> RuntimeServerControlReceipt {
        RuntimeServerControlReceipt {
            schema_id: RECEIPT_SCHEMA_ID.to_owned(),
            schema_version: SCHEMA_VERSION.to_owned(),
            request_id,
            state: self.state,
            runtime_binary_identity: self.runtime_binary_identity.clone(),
            artifact_mode: self.artifact_mode.clone(),
            artifact_catalog_digest: self.artifact_catalog_digest.clone(),
            transport_contract_digest: self.transport_contract_digest.clone(),
            workspace_entry_count: self.workspace_entry_count,
            workspace_generation: None,
            asp_python_graphs: self.asp_python_graphs.clone(),
            resident_transaction: None,
            reason: None,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeServerClientBootstrapAuthority {
    pub cwd: PathBuf,
    pub executable_path: PathBuf,
    pub state_home: PathBuf,
    pub state_home_source: agent_semantic_runtime::state_core::StateHomeResolutionSource,
    pub asp_state_home_present: bool,
    pub home_present: bool,
    pub pending_activation_path: PathBuf,
    pub applied_activation_path: PathBuf,
    pub runtime_endpoint_path: PathBuf,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeServerClientBootstrapReceipt {
    pub schema_id: &'static str,
    pub schema_version: &'static str,
    pub state: RuntimeServerState,
    pub reason_kind: &'static str,
    pub publication_nonce: Option<String>,
    pub artifact_digest: Option<String>,
    pub recommended_next: &'static str,
    pub authority: RuntimeServerClientBootstrapAuthority,
}

impl RuntimeServerClientBootstrapReceipt {
    pub const SCHEMA_ID: &'static str =
        "agent.semantic-protocols.runtime-server-client-bootstrap-receipt";
    pub const SCHEMA_VERSION: &'static str = "1";

    pub fn new(
        state: RuntimeServerState,
        reason_kind: &'static str,
        publication_nonce: Option<String>,
        artifact_digest: Option<String>,
        recommended_next: &'static str,
        authority: RuntimeServerClientBootstrapAuthority,
    ) -> Self {
        Self {
            schema_id: Self::SCHEMA_ID,
            schema_version: Self::SCHEMA_VERSION,
            state,
            reason_kind,
            publication_nonce,
            artifact_digest,
            recommended_next,
            authority,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeServerControlReceipt {
    pub schema_id: String,
    pub schema_version: String,
    pub request_id: String,
    pub state: RuntimeServerState,
    pub runtime_binary_identity: RuntimeBinaryIdentity,
    pub artifact_mode: String,
    pub artifact_catalog_digest: String,
    pub transport_contract_digest: String,
    pub workspace_entry_count: usize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workspace_generation: Option<WorkspaceGenerationControlReceipt>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub asp_python_graphs: Option<AspPythonGraphsStatus>,
    pub resident_transaction:
        Option<crate::runtime_server_owner_receipt::RuntimeServerResidentTransactionReceipt>,
    pub reason: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WorkspaceGenerationControlReceipt {
    pub workspace_identity: String,
    pub previous_generation_digest: Option<String>,
    pub active_generation_digest: String,
    pub candidate_digest: String,
    pub generation_changed: bool,
    pub state: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AspPythonGraphsStatus {
    pub state: AspPythonGraphsState,
    pub process_id: Option<u32>,
    pub runtime_artifact: Option<String>,
    pub execution_command_digest: Option<String>,
    pub reason: Option<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AspPythonGraphsState {
    Unavailable,
    Starting,
    Healthy,
    Draining,
    Failed,
}

impl RuntimeServerControlReceipt {
    pub fn healthy(
        request_id: String,
        endpoint: &RuntimeServerEndpoint,
        workspace_entry_count: usize,
    ) -> Self {
        Self {
            schema_id: RECEIPT_SCHEMA_ID.to_owned(),
            schema_version: SCHEMA_VERSION.to_owned(),
            request_id,
            state: RuntimeServerState::Healthy,
            runtime_binary_identity: endpoint.runtime_binary_identity.clone(),
            artifact_mode: endpoint.artifact_mode.clone(),
            artifact_catalog_digest: endpoint.artifact_catalog_digest.clone(),
            transport_contract_digest: endpoint.transport_contract_digest.clone(),
            workspace_entry_count,
            workspace_generation: None,
            asp_python_graphs: None,
            resident_transaction: None,
            reason: None,
        }
    }

    pub fn draining(
        request_id: String,
        endpoint: &RuntimeServerEndpoint,
        workspace_entry_count: usize,
    ) -> Self {
        Self {
            state: RuntimeServerState::Draining,
            ..Self::healthy(request_id, endpoint, workspace_entry_count)
        }
    }

    pub fn starting(
        request_id: String,
        runtime_binary_identity: RuntimeBinaryIdentity,
        artifact_mode: String,
        artifact_catalog_digest: String,
        reason: String,
    ) -> Self {
        Self {
            schema_id: RECEIPT_SCHEMA_ID.to_owned(),
            schema_version: SCHEMA_VERSION.to_owned(),
            request_id,
            state: RuntimeServerState::Starting,
            runtime_binary_identity,
            artifact_mode,
            artifact_catalog_digest,
            transport_contract_digest: runtime_server_transport_contract_digest(),
            workspace_entry_count: 0,
            workspace_generation: None,
            asp_python_graphs: None,
            resident_transaction: None,
            reason: Some(reason),
        }
    }
}

fn is_blake3_digest(value: &str) -> bool {
    value.strip_prefix("blake3-256:").is_some_and(|digest| {
        digest.len() == 64
            && digest
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    })
}

#[derive(Debug)]
pub enum RuntimeServerRequestReadError {
    Closed,
    Invalid(String),
}

impl std::fmt::Display for RuntimeServerRequestReadError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Closed => formatter.write_str("runtime server connection closed"),
            Self::Invalid(error) => formatter.write_str(error),
        }
    }
}

impl std::error::Error for RuntimeServerRequestReadError {}

fn runtime_server_transport_contract_digest_ref() -> &'static str {
    RUNTIME_SERVER_TRANSPORT_CONTRACT_DIGEST
        .get_or_init(|| {
            let mut hasher = blake3::Hasher::new();
            hasher.update(RUNTIME_SERVER_TRANSPORT_CONTRACT_DOMAIN);
            for (contract_name, contract_bytes) in [
                (
                    b"runtime-server-control".as_slice(),
                    RUNTIME_SERVER_CONTROL_CONTRACT,
                ),
                (
                    b"workspace-db-owner-ipc".as_slice(),
                    WORKSPACE_DB_OWNER_IPC_CONTRACT,
                ),
                (
                    b"runtime-server-performance-observation".as_slice(),
                    RUNTIME_PERFORMANCE_OBSERVATION_CONTRACT,
                ),
                (
                    b"runtime-server-performance-ingress-receipt".as_slice(),
                    RUNTIME_PERFORMANCE_INGRESS_RECEIPT_CONTRACT,
                ),
                (
                    b"provider-register-request".as_slice(),
                    PROVIDER_REGISTER_REQUEST_CONTRACT,
                ),
                (
                    b"provider-register-response".as_slice(),
                    PROVIDER_REGISTER_RESPONSE_CONTRACT,
                ),
                (b"asp-client-frame".as_slice(), ASP_CLIENT_FRAME_CONTRACT),
            ] {
                hasher.update(&(contract_name.len() as u64).to_le_bytes());
                hasher.update(contract_name);
                hasher.update(&(contract_bytes.len() as u64).to_le_bytes());
                hasher.update(contract_bytes);
            }
            format!("blake3-256:{}", hasher.finalize().to_hex())
        })
        .as_str()
}

#[cfg(test)]
#[path = "../../tests/unit/runtime_server_control_model.rs"]
mod supervisor_reconciliation_tests;

pub fn runtime_server_transport_contract_digest() -> String {
    runtime_server_transport_contract_digest_ref().to_owned()
}
