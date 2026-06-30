#![deny(dead_code)]

//! Runtime state materialization for ASP project-local storage.

mod graph_render;
pub mod language_owner_items;
mod provider_workspace_scope;
mod runtime_source;
pub mod state;

pub use graph_render::{
    GraphRenderReceiptRequest, run_graph_render_packet, run_graph_render_packet_bytes,
    run_graph_render_packet_bytes_with_receipt,
};
pub use language_owner_items::{
    LanguageOwnerItemsAttempt, LanguageOwnerItemsCacheRequest, LanguageOwnerItemsDispatchPlan,
    compact_language_owner_items_stdout, language_owner_items_failure, language_owner_path_exists,
    language_owner_source_path, read_language_owner_items_cache,
    run_language_owner_items_dispatch_plan, write_language_owner_items_cache,
};
pub use provider_workspace_scope::{
    PROVIDER_WORKSPACE_SCOPE_SCHEMA_ID, ProviderWorkspaceScope, ProviderWorkspaceScopeFile,
    ProviderWorkspaceScopePacket, provider_workspace_scope, provider_workspace_scope_from_stdout,
};
pub use runtime_source::{
    RuntimeSourceCheckout, RuntimeSourceIndexContext, RuntimeSourceSpec,
    ensure_runtime_source_checkout, ensure_runtime_source_checkout_in_client_cache,
    runtime_source_checkout_dir, runtime_source_checkout_dir_in_client_cache,
    runtime_source_index_context, runtime_source_registry_fingerprint,
};
pub use state::{
    ProjectRuntimeState, ProjectStatePaths, discover_project_activation_path,
    ensure_project_artifacts_dir, ensure_project_client_cache_dir, ensure_project_hook_cache_dir,
    ensure_project_hook_state_dir, ensure_project_provider_bin_dir,
    ensure_project_provider_lock_dir, ensure_project_runtime_home, is_project_activation_path,
    project_activation_path, project_cache_home, project_cache_home_for_roots,
    project_local_activation_path, project_local_client_cache_manifest_path,
    project_protocol_home_path, project_root_for_activation_path, project_runtime_state,
    project_state_paths, runtime_bin_dir_for_cache_home,
};

#[cfg(test)]
#[path = "../tests/unit/language_owner_items.rs"]
mod language_owner_items_tests;
