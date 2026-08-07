//! Typed Unix-domain transport for one workspace database owner epoch.

use std::path::PathBuf;

pub use crate::workspace_db_endpoint::{
    WorkspaceDbOwnerEndpoint, bind_workspace_db_owner, prepare_workspace_db_owner_endpoint,
    workspace_db_owner_runtime_base, workspace_db_owner_transport_contract_digest,
};
pub use crate::workspace_db_owner_election::{
    WorkspaceDbOwnerRetirement, remove_stale_workspace_db_owner_socket,
    try_retire_workspace_db_owner_endpoint,
};

pub(super) const MAX_FRAME_BYTES: usize = 8 * 1024 * 1024;

#[path = "client.rs"]
mod client;
#[path = "session.rs"]
mod session;
#[path = "protocol_types.rs"]
mod types;
pub use client::{
    cache_control_via_runtime_server, connect_runtime_server_workspace_session,
    read_source_index_via_runtime_server,
};
pub use session::WorkspaceDbIpcSession;
pub(super) use session::WorkspaceDbIpcSessionState;
pub use types::{
    RuntimeCacheControlReceipt, RuntimeCacheControlRequest, RuntimeCacheGenerationState,
    RuntimeCacheInvalidationScope, WORKSPACE_DB_OWNER_ENDPOINT_SCHEMA_ID,
    WORKSPACE_DB_OWNER_REQUEST_SCHEMA_ID, WORKSPACE_DB_OWNER_RESPONSE_SCHEMA_ID,
    WORKSPACE_DB_OWNER_SCHEMA_VERSION, WorkspaceDbIpcOperation, WorkspaceDbIpcRequest,
    WorkspaceDbIpcResponse, WorkspaceDbIpcResult, WorkspaceDbSourceIndexLookupRequest,
};

pub use crate::workspace_db_ipc_server::{
    serve_one_workspace_db_ipc_request, serve_one_workspace_db_session_request,
};

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
    pub(super) socket_path: String,
    pub(super) generation_pointer_path: Option<PathBuf>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum WorkspaceDbSessionProfile {
    Full,
    HookReadOnly,
}

#[cfg(test)]
#[path = "../../tests/unit/workspace_db_ipc.rs"]
mod tests;
