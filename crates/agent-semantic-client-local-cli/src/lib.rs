#![deny(dead_code)]

//! Local native-provider process backend for `agent-semantic-client`.

pub mod backend;
mod provider_project_scope;

pub use backend::{LocalNativeCliBackend, LocalNativeCommand, LocalNativeOutput};
pub use provider_project_scope::provider_scope_authority_permits_project_resolution;
pub use provider_project_scope::{
    ProviderProjectScope, ProviderProjectScopeFile, ProviderProjectScopeFiles,
    ProviderProjectScopePacket, ProviderProjectScopePathFile, provider_project_scope,
    provider_project_scope_files, provider_project_scope_files_async,
    provider_project_scope_files_from_packet, provider_project_scope_files_with_candidates_async,
    project_resolution_scope_from_stdout, provider_project_scope_from_stdout,
};
