#![deny(dead_code)]

//! Runtime state materialization for ASP project-local storage.

mod runtime_source;
pub mod state;

pub use runtime_source::{
    RuntimeSourceCheckout, RuntimeSourceSpec, ensure_runtime_source_checkout,
    runtime_source_checkout_dir,
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
