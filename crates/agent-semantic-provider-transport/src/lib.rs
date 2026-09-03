#![deny(dead_code)]

//! Process transport for external ASP language providers.

pub use agent_semantic_provider_protocol::{
    PROVIDER_REGISTER_REQUEST_SCHEMA_ID, PROVIDER_REGISTER_RESPONSE_SCHEMA_ID,
    PROVIDER_REGISTER_SCHEMA_VERSION, PROVIDER_SYNTAX_QUERY_OPERATION,
    PROVIDER_SYNTAX_QUERY_REQUEST_SCHEMA_ID, PROVIDER_SYNTAX_QUERY_RESPONSE_SCHEMA_ID,
    ProviderRegisterOperation, ProviderRegisterRequest, ProviderRegisterResponse,
    ProviderRegisterResult, ProviderRegisterSnapshot, ProviderRegistrationDocument,
    ProviderSchemaReference, ProviderSyntaxQueryCapture, ProviderSyntaxQueryRequest,
    ProviderSyntaxQueryResponse, SyntaxQueryPattern, SyntaxQueryPlan, SyntaxQueryPredicate,
    SyntaxQueryPredicateOp, SyntaxQueryPredicateValue,
};

#[cfg(feature = "http-client-server")]
mod asp_client_server;
mod asp_client_server_lifecycle;
pub mod byte_text;
mod capture;
mod priority_json_stream;
mod process_contract;
pub mod projection_batch;
mod resident_runtime;
mod runtime_contract;
mod runtime_process;
mod runtime_wire;
mod search_tool_process;
pub use projection_batch::{
    ProviderDerivedProjection, ProviderProjectedItem, ProviderProjectedOwner,
    ProviderProjectionBatchError, ProviderProjectionBatchRequest, ProviderProjectionBatchResponse,
    ProviderProjectionDiagnostic, ProviderProjectionOwner, ProviderProjectionState,
};
mod transport;

#[cfg(feature = "http-client-server")]
pub use agent_semantic_http_json::{
    HttpJsonRequest as AspClientServerRequest, HttpJsonResponse as AspClientServerResponse,
    run_http_json as run_asp_client_server, serve_http_json as serve_asp_client_server,
};
#[cfg(feature = "http-client-server")]
pub use asp_client_server::{AspClientServerPeer, AspClientServerSpec};
pub use asp_client_server_lifecycle::{
    ASP_CLIENT_SERVER_LIFECYCLE_RECEIPT_SCHEMA_ID, AspClientServerLifecycleReceipt,
    AspClientServerLifecycleState,
};
pub use priority_json_stream::{PriorityJsonStream, PriorityJsonStreamTerminal};
pub use process_contract::{
    DEFAULT_PROVIDER_MEMORY_LIMIT_BYTES, OutputFraming, OutputMode, ProviderProcessError,
    ProviderProcessFraming, ProviderProcessLimits, ProviderProcessReceipt, ProviderProcessSpec,
    StdinMode,
};
pub use resident_runtime::{
    ProviderRuntimeActorAuthority, ProviderRuntimeActorClient, ProviderRuntimeActorState,
    ProviderRuntimePeer, spawn_in_process_provider_runtime_actor,
    spawn_provider_runtime_peer_actor,
};
pub use runtime_contract::{
    ProviderRuntimeContractOperation, ProviderRuntimeContractReceipt,
    ProviderRuntimeContractTransport,
};
pub use runtime_process::{ProviderRuntimeProcessPeer, ProviderRuntimeProcessSpec};
pub use runtime_wire::{
    PROVIDER_RUNTIME_FRAME_SCHEMA_VERSION, PROVIDER_RUNTIME_REQUEST_FRAME_SCHEMA_ID,
    PROVIDER_RUNTIME_RESPONSE_FRAME_SCHEMA_ID, ProviderRuntimeRequestFrame,
    ProviderRuntimeResponseFrame, ProviderRuntimeResponseOutcome,
};
pub use search_tool_process::{
    FdInventoryOutput, FdInventoryReceipt, RgColdQueryOutput, RgColdQueryReceipt,
    ValidatedColdRgCorpus, run_fd_inventory, run_rg_cold_query,
};
pub use transport::{
    ProviderProcessOutput, ProviderProcessStarted, ProviderProcessSupervisor,
    provider_process_limits_from_environment,
};
#[cfg(feature = "grpc-session")]
pub mod grpc_session;
#[cfg(feature = "grpc-session")]
pub use grpc_session::GrpcProviderSessionClient;
