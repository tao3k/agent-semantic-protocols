#![deny(dead_code)]
// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Process transport for external ASP language providers.

pub use agent_semantic_provider_protocol::PROVIDER_REGISTER_REQUEST_SCHEMA_ID;
pub use agent_semantic_provider_protocol::PROVIDER_REGISTER_RESPONSE_SCHEMA_ID;
pub use agent_semantic_provider_protocol::PROVIDER_REGISTER_SCHEMA_VERSION;
pub use agent_semantic_provider_protocol::ProviderRegisterOperation;
pub use agent_semantic_provider_protocol::ProviderRegisterRequest;
pub use agent_semantic_provider_protocol::ProviderRegisterResponse;
pub use agent_semantic_provider_protocol::ProviderRegisterResult;
pub use agent_semantic_provider_protocol::ProviderRegisterSnapshot;
pub use agent_semantic_provider_protocol::ProviderRegistrationDocument;
pub use agent_semantic_provider_protocol::ProviderSchemaReference;

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
pub use projection_batch::ProviderDerivedProjection;
pub use projection_batch::ProviderProjectedItem;
pub use projection_batch::ProviderProjectedOwner;
pub use projection_batch::ProviderProjectionBatchError;
pub use projection_batch::ProviderProjectionBatchRequest;
pub use projection_batch::ProviderProjectionBatchResponse;
pub use projection_batch::ProviderProjectionDiagnostic;
pub use projection_batch::ProviderProjectionOwner;
pub use projection_batch::ProviderProjectionState;
mod transport;

#[cfg(feature = "http-client-server")]
pub use agent_semantic_http_json::HttpJsonRequest as AspClientServerRequest;
#[cfg(feature = "http-client-server")]
pub use agent_semantic_http_json::HttpJsonResponse as AspClientServerResponse;
#[cfg(feature = "http-client-server")]
pub use agent_semantic_http_json::run_http_json as run_asp_client_server;
#[cfg(feature = "http-client-server")]
pub use agent_semantic_http_json::serve_http_json as serve_asp_client_server;
#[cfg(feature = "http-client-server")]
pub use asp_client_server::AspClientServerPeer;
#[cfg(feature = "http-client-server")]
pub use asp_client_server::AspClientServerSpec;
pub use asp_client_server_lifecycle::ASP_CLIENT_SERVER_LIFECYCLE_RECEIPT_SCHEMA_ID;
pub use asp_client_server_lifecycle::AspClientServerLifecycleReceipt;
pub use asp_client_server_lifecycle::AspClientServerLifecycleState;
pub use priority_json_stream::PriorityJsonStream;
pub use priority_json_stream::PriorityJsonStreamTerminal;
pub use process_contract::DEFAULT_PROVIDER_MEMORY_LIMIT_BYTES;
pub use process_contract::OutputFraming;
pub use process_contract::OutputMode;
pub use process_contract::ProviderProcessError;
pub use process_contract::ProviderProcessFraming;
pub use process_contract::ProviderProcessLimits;
pub use process_contract::ProviderProcessReceipt;
pub use process_contract::ProviderProcessSpec;
pub use process_contract::StdinMode;
pub use resident_runtime::ProviderRuntimeActorAuthority;
pub use resident_runtime::ProviderRuntimeActorClient;
pub use resident_runtime::ProviderRuntimeActorState;
pub use resident_runtime::ProviderRuntimePeer;
pub use resident_runtime::spawn_in_process_provider_runtime_actor;
pub use resident_runtime::spawn_provider_runtime_peer_actor;
pub use runtime_contract::ProviderRuntimeContractOperation;
pub use runtime_contract::ProviderRuntimeContractReceipt;
pub use runtime_contract::ProviderRuntimeContractTransport;
pub use runtime_process::ProviderRuntimeProcessPeer;
pub use runtime_process::ProviderRuntimeProcessSpec;
pub use runtime_wire::PROVIDER_RUNTIME_FRAME_SCHEMA_VERSION;
pub use runtime_wire::PROVIDER_RUNTIME_REQUEST_FRAME_SCHEMA_ID;
pub use runtime_wire::PROVIDER_RUNTIME_RESPONSE_FRAME_SCHEMA_ID;
pub use runtime_wire::ProviderRuntimeRequestFrame;
pub use runtime_wire::ProviderRuntimeResponseFrame;
pub use runtime_wire::ProviderRuntimeResponseOutcome;
pub use transport::ProviderProcessOutput;
pub use transport::ProviderProcessStarted;
pub use transport::ProviderProcessSupervisor;
pub use transport::provider_process_limits_from_environment;
