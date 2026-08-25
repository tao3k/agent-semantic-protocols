#![deny(dead_code)]

//! Tokio client lifecycle and request contracts for ASP language servers.

mod client_protocol;
mod http_client;
mod provider_project_resolution;

pub use agent_semantic_http_json::{
    HttpJsonRequest, HttpJsonResponse, run_http_json, serve_http_json, serve_http_json_h2,
};
pub use client_protocol::{
    AspClientCancelFuture, AspClientDispatchError, AspClientDispatchFuture,
    AspClientDispatchRequest, AspClientDispatcher, AspClientProtocolHttpService,
    serve_asp_client_protocol_http,
};
pub use http_client::AspClientProtocolHttpClient;

pub use provider_project_resolution::provider_capabilities_permit_project_resolution;
pub use provider_project_resolution::{
    ProviderProjectResolution, ProviderProjectResolutionCandidates,
    ProviderProjectResolutionCollectionScope, ProviderProjectResolutionFile,
    ProviderProjectResolutionFiles, ProviderProjectResolutionPacket,
    ProviderProjectResolutionPathFile, ProviderProjectResolutionPolicyExclusion,
    encode_provider_project_resolution_request, project_resolution_from_stdout,
    provider_project_resolution_candidates, provider_project_resolution_files_from_packet,
    provider_project_resolution_files_from_packet_async,
};
