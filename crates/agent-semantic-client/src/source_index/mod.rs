//! DB Engine-owned source index refresh and lookup facade.

mod api;
mod async_rebuild;
mod async_snapshot;
mod collect;
mod generation_build;
mod generation_commit;
mod generation_overlay;
mod projection;
mod provider_envelope;

#[cfg(test)]
pub(crate) use api::materialized_current_source_index_snapshot;
pub use api::{CurrentSourceIndexSnapshot, current_provider_source_index_snapshot_with_registry};
pub use async_rebuild::{
    prepare_runtime_server_owner_projection_with_resident_runtime_async,
    prepare_runtime_server_workspace_generation_with_runtime_service_async,
};

pub use provider_envelope::{
    ProviderSourceEnvelopeLookupRequestV1, ProviderSourceSnapshotEnvelopePublicationV1,
    ProviderWorkspaceIdentityV1,
    current_provider_source_index_snapshot_at_artifact_root_with_registry,
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

#[cfg(test)]
pub(crate) use lookup::search_pipe_source_index_lookup_from_client_result;
pub use lookup::{
    SourceIndexLookupRequest, lookup_search_pipe_source_index_for_language, lookup_source_index,
    lookup_source_index_for_language, lookup_source_index_in_cache,
};
pub use model::{
    SourceIndexCandidate, SourceIndexLookupResult, SourceIndexLookupState,
    SourceIndexRefreshReport, SourceIndexSourceKind,
};
