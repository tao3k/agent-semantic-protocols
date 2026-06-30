#![deny(dead_code)]

//! Local native-provider process backend for `agent-semantic-client`.

pub mod backend;
mod provider_workspace_scope;

pub use backend::{LocalNativeCliBackend, LocalNativeCommand, LocalNativeOutput};
pub use provider_workspace_scope::{
    PROVIDER_WORKSPACE_SCOPE_SCHEMA_ID, ProviderWorkspaceScope, ProviderWorkspaceScopeFile,
    ProviderWorkspaceScopeFiles, ProviderWorkspaceScopePacket, ProviderWorkspaceScopePathFile,
    collect_provider_source_scope_files, provider_workspace_scope, provider_workspace_scope_files,
    provider_workspace_scope_files_from_packet, provider_workspace_scope_from_stdout,
};
