#![deny(dead_code)]

//! Process transport for external ASP language providers.

pub use agent_semantic_provider_protocol::{
    PROVIDER_REGISTER_REQUEST_SCHEMA_ID, PROVIDER_REGISTER_RESPONSE_SCHEMA_ID,
    PROVIDER_REGISTER_SCHEMA_VERSION, PROVIDER_SYNTAX_QUERY_OPERATION,
    PROVIDER_SYNTAX_QUERY_REQUEST_SCHEMA_ID, PROVIDER_SYNTAX_QUERY_RESPONSE_SCHEMA_ID,
    ProviderRegisterOperation, ProviderRegisterRequest, ProviderRegisterResponse,
    ProviderRegisterResult, ProviderRegisterSnapshot, ProviderRegistrationDocument,
    ProviderSyntaxQueryCapture, ProviderSyntaxQueryRequest, ProviderSyntaxQueryResponse,
    SyntaxQueryPattern, SyntaxQueryPlan, SyntaxQueryPredicate, SyntaxQueryPredicateOp,
    SyntaxQueryPredicateValue,
};

mod asp_client_server;
mod asp_client_server_host;
mod asp_client_server_lifecycle;
pub mod byte_text;
mod capture;
mod process_contract;
pub mod projection_batch;
mod resident_runtime;
mod runtime_contract;
mod runtime_process;
mod runtime_wire;
pub use projection_batch::{
    ProviderDerivedProjection, ProviderProjectedItem, ProviderProjectedOwner,
    ProviderProjectionBatchError, ProviderProjectionBatchRequest, ProviderProjectionBatchResponse,
    ProviderProjectionOwner,
};
mod transport;

pub use asp_client_server::{AspClientServerPeer, AspClientServerSpec};
pub use asp_client_server_host::{
    AspClientServerRequest, AspClientServerResponse, run_asp_client_server, serve_asp_client_server,
};
pub use asp_client_server_lifecycle::{
    ASP_CLIENT_SERVER_LIFECYCLE_RECEIPT_SCHEMA_ID, AspClientServerLifecycleReceipt,
    AspClientServerLifecycleState,
};
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
pub use transport::{
    ProviderProcessOutput, ProviderProcessSupervisor, provider_process_limits_from_environment,
};
