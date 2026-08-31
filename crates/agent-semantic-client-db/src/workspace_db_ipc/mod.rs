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
pub(crate) use transport::{read_frame, runtime_server_data_terminal_error, write_frame};
use validation::{
    deserialize_changed_paths, deserialize_mutation_id, workspace_db_ipc_read_lane_capacity,
};

pub use agent_session_registry::{
    AgentHostEventId, AgentHostExecutionObservationIpc, AgentHostLifecycleEventIpc,
    AgentHostLifecycleEventKind, AgentHostNamespaceId, AgentHostNonMatchIpc, AgentHostProfileId,
    AgentHostRouteKey, AgentHostSandboxMode, AgentHostTranscriptPath,
    AgentSessionModelObservationIpc, AgentSessionRegisterIpcRequest,
    AgentSessionRegistryIpcOperation, AgentSessionRegistryIpcResult,
};
pub use graph_facts::{RuntimeGraphFactSource, RuntimeGraphFactsRead};
pub use protocol::{
    RuntimeCacheControlReceipt, RuntimeCacheControlRequest, RuntimeCacheGenerationState,
    RuntimeCacheInvalidationScope, RuntimeCacheOwnerDeltaFallbackPolicy,
    RuntimeResidentReadEvidence, RuntimeResidentReadResult, RuntimeResidentReadState,
    RuntimeResidentReadTerminalState, WORKSPACE_DB_OWNER_ENDPOINT_SCHEMA_ID,
    WORKSPACE_DB_OWNER_REQUEST_SCHEMA_ID, WORKSPACE_DB_OWNER_RESPONSE_SCHEMA_ID,
    WORKSPACE_DB_OWNER_SCHEMA_VERSION, WorkspaceDbIpcBindingToken, WorkspaceDbIpcOperation,
    WorkspaceDbIpcRequest, WorkspaceDbIpcRequestId, WorkspaceDbIpcResponse, WorkspaceDbIpcResult,
    WorkspaceDbIpcSchemaId, WorkspaceDbIpcSession, WorkspaceDbSourceIndexLookupRequest,
    WorkspaceIpcResidentReadWorkCounters, connect_runtime_server_workspace_session,
};
pub(crate) use protocol::{RuntimeResidentReadTerminalDigestInput, resident_read_terminal_digest};
pub use transport::{
    HOST_LOCAL_IPC_PERMISSION_DENIED_REASON_KIND, is_host_local_ipc_permission_denied,
};
