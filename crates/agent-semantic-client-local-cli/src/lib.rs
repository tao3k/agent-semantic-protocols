#![deny(dead_code)]

//! Local native-provider process backend for `agent-semantic-client`.

pub mod backend;
mod provider_workspace_scope;

pub use backend::{LocalNativeCliBackend, LocalNativeCommand, LocalNativeOutput};
pub use provider_workspace_scope::{
    ProviderWorkspaceScope, ProviderWorkspaceScopeFile, ProviderWorkspaceScopeFiles,
    ProviderWorkspaceScopePacket, ProviderWorkspaceScopePathFile, provider_workspace_scope,
    provider_workspace_scope_files, provider_workspace_scope_files_from_packet,
    provider_workspace_scope_from_stdout,
};
