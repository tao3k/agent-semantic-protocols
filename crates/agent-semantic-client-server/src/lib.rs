#![deny(dead_code)]
// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-only


//! Tokio client lifecycle and request contracts for ASP language servers.

mod client_protocol;
mod grpc;
mod provider_project_resolution;

pub use client_protocol::AspClientCancelFuture;
pub use client_protocol::AspClientDispatchError;
pub use client_protocol::AspClientDispatchFuture;
pub use client_protocol::AspClientDispatchRequest;
pub use client_protocol::AspClientDispatcher;
pub use client_protocol::AspClientFrameService;
pub use grpc::AspClientGrpcService;
pub use grpc::AspClientGrpcTransport;
pub use grpc::CLIENT_FRAME_SESSION_CAPACITY;
pub use grpc::CLIENT_FRAME_SESSION_CONTROL_RESERVE;
pub use grpc::admit_asp_client_grpc_inherited_descriptor;
pub use grpc::bind_asp_client_grpc_tcp;
pub use grpc::connect_asp_client_grpc_inherited_descriptor;
pub use grpc::serve_asp_client_grpc_tcp;

pub use provider_project_resolution::ProviderProjectResolution;
pub use provider_project_resolution::ProviderProjectResolutionCandidates;
pub use provider_project_resolution::ProviderProjectResolutionCollectionScope;
pub use provider_project_resolution::ProviderProjectResolutionFile;
pub use provider_project_resolution::ProviderProjectResolutionFiles;
pub use provider_project_resolution::ProviderProjectResolutionPacket;
pub use provider_project_resolution::ProviderProjectResolutionPathFile;
pub use provider_project_resolution::ProviderProjectResolutionPolicyExclusion;
pub use provider_project_resolution::encode_provider_project_resolution_request;
pub use provider_project_resolution::project_resolution_from_stdout;
pub use provider_project_resolution::provider_capabilities_permit_project_resolution;
pub use provider_project_resolution::provider_project_resolution_candidates;
pub use provider_project_resolution::provider_project_resolution_files_from_packet;
pub use provider_project_resolution::provider_project_resolution_files_from_packet_async;
