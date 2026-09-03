#![deny(dead_code)]

//! Tokio client lifecycle and request contracts for ASP language servers.

mod client_protocol;
mod grpc;
mod provider_project_resolution;

pub use client_protocol::{
    AspClientCancelFuture, AspClientDispatchError, AspClientDispatchFuture,
    AspClientDispatchRequest, AspClientDispatcher, AspClientFrameService,
};
pub use grpc::{
    AspClientGrpcService, AspClientGrpcTransport, CLIENT_FRAME_SESSION_CAPACITY,
    CLIENT_FRAME_SESSION_CONTROL_RESERVE, admit_asp_client_grpc_inherited_descriptor,
    bind_asp_client_grpc_tcp, connect_asp_client_grpc_inherited_descriptor,
    serve_asp_client_grpc_tcp,
};

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
