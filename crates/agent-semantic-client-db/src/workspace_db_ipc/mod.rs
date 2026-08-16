//! Runtime Server workspace data-plane protocol and transport.

mod agent_session_registry;
mod codex_control_plane;
mod graph_facts;
mod protocol;
pub use protocol::{
    RUNTIME_MERKLE_OWNER_READ_REQUEST_SCHEMA_ID, RuntimeMerkleOwnerReadRequest,
    read_runtime_merkle_owner_via_runtime_server,
};
mod provider_owner;
mod runtime_generation;
mod runtime_locator;
mod session_pool;
pub(crate) mod transport;
mod validation;

use runtime_generation::MutationWorkspaceLane;
use session_pool::resident_state;
use transport::runtime_server_data_connect_error;
pub(crate) use transport::{read_frame, read_optional_frame, write_frame};
use validation::{
    deserialize_changed_paths, deserialize_mutation_id, workspace_db_ipc_read_lane_capacity,
};

pub use crate::try_acquire_workspace_db_owner_election;
pub use agent_session_registry::{
    AgentHostExecutionObservationIpc, AgentHostLifecycleEventIpc, AgentHostLifecycleEventKind,
    AgentHostNonMatchIpc, AgentSessionModelObservationIpc, AgentSessionRegisterIpcRequest,
    AgentSessionRegistryIpcOperation, AgentSessionRegistryIpcResult,
};
pub use graph_facts::{RuntimeGraphFactSource, RuntimeGraphFactsRead};
pub use protocol::{
    RuntimeCacheControlReceipt, RuntimeCacheControlRequest, RuntimeCacheGenerationState,
    RuntimeCacheInvalidationScope, RuntimeCacheOwnerDeltaFallbackPolicy,
    WORKSPACE_DB_OWNER_ENDPOINT_SCHEMA_ID, WORKSPACE_DB_OWNER_REQUEST_SCHEMA_ID,
    WORKSPACE_DB_OWNER_RESPONSE_SCHEMA_ID, WORKSPACE_DB_OWNER_SCHEMA_VERSION,
    WorkspaceDbIpcOperation, WorkspaceDbIpcRequest, WorkspaceDbIpcResponse, WorkspaceDbIpcResult,
    WorkspaceDbIpcSession, WorkspaceDbOwnerEndpoint, WorkspaceDbOwnerRetirement,
    WorkspaceDbSourceIndexLookupRequest, bind_workspace_db_owner, cache_control_via_runtime_server,
    connect_runtime_server_workspace_session, prepare_workspace_db_owner_endpoint,
    read_source_index_via_runtime_server, remove_stale_workspace_db_owner_socket,
    serve_one_workspace_db_ipc_request, serve_one_workspace_db_session_request,
    try_retire_workspace_db_owner_endpoint, workspace_db_owner_runtime_base,
    workspace_db_owner_transport_contract_digest,
};
pub use transport::{
    HOST_LOCAL_IPC_PERMISSION_DENIED_REASON_KIND, call_workspace_db_owner,
    is_host_local_ipc_permission_denied,
};
