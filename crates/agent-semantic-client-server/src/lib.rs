#![deny(dead_code)]

//! Tokio client lifecycle and request contracts for ASP language servers.

mod client_protocol;
mod grpc;
mod http;
mod provider_project_resolution;

pub use client_protocol::{
    AspClientCancelFuture, AspClientDispatchError, AspClientDispatchFuture,
    AspClientDispatchRequest, AspClientDispatcher, AspClientFrameService,
};
pub use grpc::{
    AspClientGrpcService, AspClientGrpcTransport, bind_asp_client_grpc_unix,
    serve_asp_client_grpc_unix,
};
pub use http::{ASP_CLIENT_HTTP_FRAME_PATH, serve_asp_client_http_json};

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
