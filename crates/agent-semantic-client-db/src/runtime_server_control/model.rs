use std::path::Path;
use std::sync::OnceLock;

use serde::{Deserialize, Serialize};

pub(super) const SCHEMA_VERSION: &str = "1";
pub(super) const ENDPOINT_SCHEMA_ID: &str = "agent.semantic-protocols.runtime-server-endpoint.v1";
pub(super) const REQUEST_SCHEMA_ID: &str =
    "agent.semantic-protocols.runtime-server-control-request.v1";
pub(super) const RECEIPT_SCHEMA_ID: &str =
    "agent.semantic-protocols.runtime-server-control-receipt.v1";
const STATUS_SNAPSHOT_SCHEMA_ID: &str =
    "agent.semantic-protocols.runtime-server-status-snapshot.v1";

static RUNTIME_SERVER_TRANSPORT_CONTRACT_DIGEST: OnceLock<String> = OnceLock::new();
const RUNTIME_SERVER_TRANSPORT_CONTRACT_DOMAIN: &[u8] =
    b"agent.semantic-protocols.runtime-server-transport.v1";
const RUNTIME_SERVER_CONTROL_CONTRACT: &[u8] =
    include_bytes!("../../../../schemas/runtime-server-control.v1.schema.json");
const WORKSPACE_DB_OWNER_IPC_CONTRACT: &[u8] =
    include_bytes!("../../../../schemas/workspace-db-owner-ipc.v1.schema.json");

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeServerEndpoint {
    pub schema_id: String,
    pub schema_version: String,
    pub transport_contract_digest: String,
    pub owner_epoch: u64,
    pub runtime_artifact_path: String,
    pub runtime_artifact_digest: String,
    pub artifact_mode: String,
    pub artifact_catalog_digest: String,
    pub binding_token: String,
    pub socket_path: String,
    pub data_plane_socket_path: String,
    pub workspace_store_path: String,
    pub status_memory_path: String,
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
            || self.runtime_artifact_digest.is_empty()
            || !matches!(self.artifact_mode.as_str(), "dev" | "release")
            || !is_blake3_digest(&self.artifact_catalog_digest)
            || self.binding_token.is_empty()
            || self.socket_path.is_empty()
            || self.data_plane_socket_path.is_empty()
            || self.workspace_store_path.is_empty()
            || self.status_memory_path.is_empty()
        {
            return Err("Runtime Server endpoint is incomplete".to_owned());
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
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum RuntimeServerOperation {
    Status,
    Reconcile,
    Restart,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RuntimeServerControlRequest {
    pub schema_id: String,
    pub schema_version: String,
    pub operation: RuntimeServerOperation,
    pub expected_runtime_artifact_digest: String,
    pub request_id: String,
    pub transport_contract_digest: String,
    pub owner_epoch: u64,
    pub binding_token: String,
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
            || self.expected_runtime_artifact_digest.is_empty()
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
            RuntimeServerOperation::Reconcile => Ok(self.expected_runtime_artifact_digest
                != endpoint.runtime_artifact_digest
                || self.transport_contract_digest != endpoint.transport_contract_digest),
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
    pub runtime_artifact_digest: String,
    pub artifact_mode: String,
    pub artifact_catalog_digest: String,
    pub transport_contract_digest: String,
    pub workspace_entry_count: usize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub graph_turbo_resident: Option<GraphTurboResidentStatus>,
    pub owner_epoch: u64,
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
            runtime_artifact_digest: endpoint.runtime_artifact_digest.clone(),
            artifact_mode: endpoint.artifact_mode.clone(),
            artifact_catalog_digest: endpoint.artifact_catalog_digest.clone(),
            transport_contract_digest: endpoint.transport_contract_digest.clone(),
            workspace_entry_count,
            graph_turbo_resident: None,
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
            || self.runtime_artifact_digest != endpoint.runtime_artifact_digest
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
            runtime_artifact_digest: self.runtime_artifact_digest.clone(),
            artifact_mode: self.artifact_mode.clone(),
            artifact_catalog_digest: self.artifact_catalog_digest.clone(),
            transport_contract_digest: self.transport_contract_digest.clone(),
            workspace_entry_count: self.workspace_entry_count,
            graph_turbo_resident: self.graph_turbo_resident.clone(),
            reason: None,
        })
    }

    pub(crate) fn cached_health_receipt(&self, request_id: String) -> RuntimeServerControlReceipt {
        RuntimeServerControlReceipt {
            schema_id: RECEIPT_SCHEMA_ID.to_owned(),
            schema_version: SCHEMA_VERSION.to_owned(),
            request_id,
            state: self.state,
            runtime_artifact_digest: self.runtime_artifact_digest.clone(),
            artifact_mode: self.artifact_mode.clone(),
            artifact_catalog_digest: self.artifact_catalog_digest.clone(),
            transport_contract_digest: self.transport_contract_digest.clone(),
            workspace_entry_count: self.workspace_entry_count,
            graph_turbo_resident: self.graph_turbo_resident.clone(),
            reason: None,
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
    pub runtime_artifact_digest: String,
    pub artifact_mode: String,
    pub artifact_catalog_digest: String,
    pub transport_contract_digest: String,
    pub workspace_entry_count: usize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub graph_turbo_resident: Option<GraphTurboResidentStatus>,
    pub reason: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GraphTurboResidentStatus {
    pub state: GraphTurboResidentState,
    pub process_id: Option<u32>,
    pub runtime_artifact: Option<String>,
    pub execution_command_digest: Option<String>,
    pub reason: Option<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum GraphTurboResidentState {
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
            runtime_artifact_digest: endpoint.runtime_artifact_digest.clone(),
            artifact_mode: endpoint.artifact_mode.clone(),
            artifact_catalog_digest: endpoint.artifact_catalog_digest.clone(),
            transport_contract_digest: endpoint.transport_contract_digest.clone(),
            workspace_entry_count,
            graph_turbo_resident: None,
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
        runtime_artifact_digest: String,
        artifact_mode: String,
        artifact_catalog_digest: String,
        reason: String,
    ) -> Self {
        Self {
            schema_id: RECEIPT_SCHEMA_ID.to_owned(),
            schema_version: SCHEMA_VERSION.to_owned(),
            request_id,
            state: RuntimeServerState::Starting,
            runtime_artifact_digest,
            artifact_mode,
            artifact_catalog_digest,
            transport_contract_digest: runtime_server_transport_contract_digest(),
            workspace_entry_count: 0,
            graph_turbo_resident: None,
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
                    b"runtime-server-control.v1".as_slice(),
                    RUNTIME_SERVER_CONTROL_CONTRACT,
                ),
                (
                    b"workspace-db-owner-ipc.v1".as_slice(),
                    WORKSPACE_DB_OWNER_IPC_CONTRACT,
                ),
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
