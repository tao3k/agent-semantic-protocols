//! DB Engine-owned source index refresh and lookup facade.

mod api;
mod async_rebuild;
mod async_snapshot;
mod collect;
mod generation_build;
mod generation_commit;
pub use api::current_live_provider_source_index_snapshot_with_registry;
mod projection;
mod provider_envelope;

pub(crate) use api::current_runtime_source_index_snapshot;
pub(crate) use api::current_source_index_snapshot_with_registry;
#[cfg(test)]
pub(crate) use api::materialized_current_source_index_snapshot;
pub use api::rebuild_source_index;
pub use api::{
    CurrentSourceIndexSnapshot, current_provider_source_index_snapshot_with_registry,
    current_source_index_snapshot, current_source_index_snapshot_for_owner,
    current_source_index_snapshot_for_owner_from_activation,
    current_workspace_search_source_index_snapshot,
};
pub use async_rebuild::{rebuild_source_index_async, rebuild_source_index_with_registry_async};
pub use projection::{LanguageProjectionImportReport, import_language_projection};
pub use provider_envelope::{
    ProviderSourceEnvelopeLookupRequestV1, ProviderSourceSnapshotEnvelopePublicationV1,
    ProviderWorkspaceIdentityV1,
    current_provider_source_index_snapshot_at_artifact_root_with_registry,
    ensure_provider_source_index_snapshot_at_artifact_root_with_registry,
    provider_source_snapshot_envelope_path_at_artifact_root_with_registry,
    provider_workspace_identity_v1, publish_provider_source_snapshot_envelope,
};
mod generation;

pub use collect::SourceIndexCollectionScope;
pub use generation::{
    PublishedSourceIndexGenerationV1, TargetProviderSourceEnvelopePublicationRequestV1,
    WorkspaceSearchGenerationPublicationRequestV1, publish_target_provider_source_envelope_v1,
    publish_workspace_search_generation_v1,
};
mod config;
mod lookup;
mod model;

pub use api::CurrentSourceIndexOwnerFromActivationRequest;
pub use api::{
    current_provider_source_index_snapshot_from_activation,
    current_source_index_snapshot_from_activation,
    ensure_provider_source_index_snapshot_from_activation,
    provider_source_snapshot_envelope_path_from_activation,
};
pub use api::{refresh_runtime_source_index, refresh_source_index};
#[cfg(test)]
pub(crate) use lookup::search_pipe_source_index_lookup_from_client_result;
pub use lookup::{
    SourceIndexClientCacheLookupRequest, SourceIndexLookupRequest,
    lookup_search_pipe_source_index_for_language, lookup_source_index,
    lookup_source_index_for_language, lookup_source_index_in_cache,
    lookup_source_index_in_client_cache_dir,
};
pub use model::{
    SourceIndexCandidate, SourceIndexLookupResult, SourceIndexLookupState,
    SourceIndexRefreshReport, SourceIndexSourceKind,
};
