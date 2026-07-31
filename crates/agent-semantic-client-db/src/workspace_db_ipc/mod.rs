//! Runtime Server workspace data-plane protocol and transport.

mod protocol;
mod provider_owner;
mod runtime_generation;
pub(crate) mod transport;

pub use protocol::{
    WORKSPACE_DB_OWNER_ENDPOINT_SCHEMA_ID, WORKSPACE_DB_OWNER_REQUEST_SCHEMA_ID,
    WORKSPACE_DB_OWNER_RESPONSE_SCHEMA_ID, WORKSPACE_DB_OWNER_SCHEMA_VERSION,
    WorkspaceDbIpcOperation, WorkspaceDbIpcRequest, WorkspaceDbIpcResponse, WorkspaceDbIpcResult,
    WorkspaceDbIpcSession, WorkspaceDbOwnerEndpoint, WorkspaceDbOwnerRetirement,
    WorkspaceDbSourceIndexLookupRequest, bind_workspace_db_owner,
    connect_runtime_server_workspace_session, prepare_workspace_db_owner_endpoint,
    read_source_index_via_runtime_server, remove_stale_workspace_db_owner_socket,
    serve_one_workspace_db_ipc_request, serve_one_workspace_db_session_request,
    serve_workspace_db_session_until_shutdown, try_acquire_workspace_db_owner_election,
    try_retire_workspace_db_owner_endpoint, workspace_db_owner_runtime_base,
    workspace_db_owner_transport_contract_digest,
};
pub use transport::call_workspace_db_owner;
