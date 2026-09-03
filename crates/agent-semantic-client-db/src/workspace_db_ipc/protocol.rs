//! Typed loopback transport for one workspace database owner epoch.

use std::path::PathBuf;

pub(super) const MAX_FRAME_BYTES: usize = 8 * 1024 * 1024;

#[path = "client.rs"]
mod client;
#[path = "session.rs"]
mod session;
#[path = "protocol_types.rs"]
mod types;
pub use client::{
    connect_runtime_server_workspace_session, read_runtime_merkle_owner_via_runtime_server,
};
pub use session::WorkspaceDbIpcSession;
pub(super) use session::WorkspaceDbIpcSessionState;
pub use types::{RUNTIME_MERKLE_OWNER_READ_REQUEST_SCHEMA_ID, RuntimeMerkleOwnerReadRequest};
pub use types::{
    RuntimeCacheControlReceipt, RuntimeCacheControlRequest, RuntimeCacheGenerationState,
    RuntimeCacheInvalidationScope, RuntimeCacheOwnerDeltaFallbackPolicy, RuntimeResidentReadState,
    RuntimeResidentReadTerminalState, WORKSPACE_DB_OWNER_ENDPOINT_SCHEMA_ID,
    WORKSPACE_DB_OWNER_REQUEST_SCHEMA_ID, WORKSPACE_DB_OWNER_RESPONSE_SCHEMA_ID,
    WORKSPACE_DB_OWNER_SCHEMA_VERSION, WorkspaceDbIpcBindingToken, WorkspaceDbIpcOperation,
    WorkspaceDbIpcRequest, WorkspaceDbIpcRequestId, WorkspaceDbIpcResponse, WorkspaceDbIpcResult,
    WorkspaceDbIpcSchemaId, WorkspaceDbSourceIndexLookupRequest,
};
pub use types::{
    RuntimeResidentReadEvidence, RuntimeResidentReadResult, WorkspaceIpcResidentReadWorkCounters,
};
pub(crate) use types::{RuntimeResidentReadTerminalDigestInput, resident_read_terminal_digest};

static NEXT_WORKSPACE_DB_IPC_CLIENT_ID: std::sync::atomic::AtomicU64 =
    std::sync::atomic::AtomicU64::new(1);

#[derive(Clone, Debug)]
pub(super) struct WorkspaceDbSessionBinding {
    pub(super) workspace_identity: String,
    pub(super) project_root: Option<PathBuf>,
    pub(super) transport_contract_digest: String,
    pub(super) owner_epoch: u64,
    pub(super) runtime_binary_path: String,
    pub(super) runtime_binary_digest: String,
    pub(super) binding_token: String,
    pub(super) data_endpoint: crate::runtime_server_control::RuntimeServerLoopbackEndpoint,
    pub(super) generation_pointer_path: Option<PathBuf>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum WorkspaceDbSessionProfile {
    Full,
    HookReadOnly,
}
