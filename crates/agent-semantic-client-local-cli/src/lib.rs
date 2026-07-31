#![deny(dead_code)]

//! Local native-provider process backend for `agent-semantic-client`.

pub mod backend;
mod provider_project_resolution;

pub use backend::{LocalNativeCliBackend, LocalNativeCommand, LocalNativeOutput};
pub use provider_project_resolution::provider_scope_authority_permits_project_resolution;
pub use provider_project_resolution::{
    ProviderProjectResolution, ProviderProjectResolutionCandidates, ProviderProjectResolutionFile,
    ProviderProjectResolutionFiles, ProviderProjectResolutionPacket,
    ProviderProjectResolutionPathFile, ProviderProjectResolutionPolicyExclusion,
    encode_provider_project_resolution_request, project_resolution_from_stdout,
    provider_project_resolution_candidates, provider_project_resolution_files_from_packet,
    provider_project_resolution_files_from_packet_async,
    provider_project_resolution_files_with_candidates,
    provider_project_resolution_files_with_candidates_async,
    provider_project_resolution_with_candidates, provider_project_resolution_with_candidates_async,
};
