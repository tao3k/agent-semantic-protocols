//! DB Engine-owned source index refresh and lookup facade.

mod api;
mod projection;

pub(crate) use api::current_runtime_source_index_snapshot;
pub(crate) use api::current_source_index_snapshot_with_registry;
pub use api::rebuild_source_index;
pub use api::{
    CurrentSourceIndexSnapshot, current_source_index_snapshot,
    publish_provider_source_snapshot_envelope,
};
pub use projection::{LanguageProjectionImportReport, import_language_projection};
mod collect;
mod config;
mod lookup;
mod model;

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
