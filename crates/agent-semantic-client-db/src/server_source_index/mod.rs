// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! ASP Server-owned Source Index generation, provider projection, and IPC-adjacent
//! publication orchestration. Durable rows and Merkle state live in this DB crate.

mod api;
mod async_rebuild;
mod async_snapshot;
mod collect;
mod config;
mod generation;
mod generation_build;
mod generation_commit;
mod generation_overlay;
mod model;
mod projection;
mod provider_envelope;

#[cfg(test)]
pub(crate) use api::materialized_current_source_index_snapshot;
pub use api::{CurrentSourceIndexSnapshot, current_provider_source_index_snapshot_with_registry};
pub use async_rebuild::{
    prepare_runtime_server_owner_projection_with_resident_runtime_async,
    prepare_runtime_server_workspace_generation_with_runtime_service_async,
};
pub use collect::SourceIndexCollectionScope;
pub use generation::{
    PublishedSourceIndexGenerationV1, WorkspaceSearchGenerationPublicationRequestV1,
    publish_workspace_search_generation_v1,
};
pub use model::{
    SourceIndexCandidate, SourceIndexLookupResult, SourceIndexLookupState,
    SourceIndexRefreshReport, SourceIndexScopeFile, SourceIndexSourceKind,
};
pub use provider_envelope::{
    ProviderSourceEnvelopeLookupRequestV1, ProviderSourceSnapshotEnvelopePublicationV1,
    ProviderWorkspaceIdentityV1,
    current_provider_source_index_snapshot_at_artifact_root_with_registry,
    provider_source_snapshot_envelope_path_at_artifact_root_with_registry,
    provider_workspace_identity_v1, publish_provider_source_snapshot_envelope,
};
