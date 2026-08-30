//! Installed provider artifact branch boundary.

mod core;

pub(crate) use core::{
    InstalledProviderArtifactsPublication, RuntimeProviderArtifacts,
    load_runtime_provider_artifacts, publish_current_installed_provider_artifacts,
    runtime_source_index_provider_projection, runtime_source_index_provider_projection_for_target,
};
