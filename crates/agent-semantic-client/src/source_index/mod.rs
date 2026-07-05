//! DB Engine-owned source index refresh and lookup facade.

mod api;
mod collect;
mod config;
mod lookup;
mod model;

pub use api::{refresh_runtime_source_index, refresh_source_index};
pub use lookup::{
    SourceIndexClientCacheLookupRequest, SourceIndexLookupRequest,
    lookup_query_wrapper_source_index, lookup_search_pipe_source_index_for_language,
    lookup_source_index, lookup_source_index_for_language, lookup_source_index_in_cache,
    lookup_source_index_in_client_cache_dir,
};
#[cfg(test)]
pub(crate) use lookup::{
    query_wrapper_source_index_lookup_from_client_result,
    search_pipe_source_index_lookup_from_client_result,
};
pub use model::{
    SourceIndexCandidate, SourceIndexLookupResult, SourceIndexLookupState,
    SourceIndexRefreshReport, SourceIndexSourceKind,
};
