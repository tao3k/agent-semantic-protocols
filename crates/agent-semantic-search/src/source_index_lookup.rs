//! DB-backed source-index lookup adapter.

use std::path::Path;

use agent_semantic_client_core::LanguageId;
use agent_semantic_client_db::{
    ClientDbSourceIndexClientDirLookupRequest, ClientDbSourceIndexLookupResult,
    ClientDbSourceIndexProjectLookupRequest, ClientDbSourceIndexQueryKey,
    lookup_source_index_from_client_dir, lookup_source_index_from_project,
};

use crate::{reorder_source_index_candidates, source_index_lookup_terms};

/// Request for looking up source-index owners from one project's cache.
pub struct SourceIndexLookupRequest<'a> {
    pub cache_project_root: &'a Path,
    pub indexed_project_root: &'a Path,
    pub language_id: Option<&'a LanguageId>,
    pub query: &'a str,
    pub limit: u32,
}

/// Request for looking up source-index owners from an already resolved client
/// cache directory.
pub struct SourceIndexClientCacheLookupRequest<'a> {
    pub cache_root: &'a Path,
    pub indexed_project_root: &'a Path,
    pub language_id: Option<&'a LanguageId>,
    pub query: &'a str,
    pub limit: u32,
}

/// Lookup source-index owners from the client DB for one project root.
pub fn lookup_source_index(
    project_root: &Path,
    query: &str,
    limit: u32,
) -> Result<ClientDbSourceIndexLookupResult, String> {
    lookup_source_index_for_language(project_root, None, query, limit)
}

/// Lookup source-index owners from the client DB for one language scope.
pub fn lookup_source_index_for_language(
    project_root: &Path,
    language_id: Option<&LanguageId>,
    query: &str,
    limit: u32,
) -> Result<ClientDbSourceIndexLookupResult, String> {
    lookup_source_index_in_cache(SourceIndexLookupRequest {
        cache_project_root: project_root,
        indexed_project_root: project_root,
        language_id,
        query,
        limit,
    })
}

/// Lookup source-index owners from one project's client DB for an explicit
/// indexed project root.
pub fn lookup_source_index_in_cache(
    request: SourceIndexLookupRequest<'_>,
) -> Result<ClientDbSourceIndexLookupResult, String> {
    let lookup = lookup_source_index_from_project(ClientDbSourceIndexProjectLookupRequest {
        cache_project_root: request.cache_project_root,
        indexed_project_root: request.indexed_project_root,
        language_id: request.language_id,
        query_keys: source_index_lookup_query_keys(request.query),
        limit: request.limit,
    })?;
    Ok(rank_source_index_lookup_result(lookup, request.query))
}

/// Lookup source-index owners from an already resolved client cache directory.
pub fn lookup_source_index_in_client_cache_dir(
    request: SourceIndexClientCacheLookupRequest<'_>,
) -> Result<ClientDbSourceIndexLookupResult, String> {
    let lookup = lookup_source_index_from_client_dir(ClientDbSourceIndexClientDirLookupRequest {
        client_dir: request.cache_root,
        indexed_project_root: request.indexed_project_root,
        language_id: request.language_id,
        query_keys: source_index_lookup_query_keys(request.query),
        limit: request.limit,
    })?;
    Ok(rank_source_index_lookup_result(lookup, request.query))
}

fn source_index_lookup_query_keys(query: &str) -> Vec<ClientDbSourceIndexQueryKey> {
    source_index_lookup_terms(query)
        .into_iter()
        .map(ClientDbSourceIndexQueryKey::from)
        .collect()
}

fn rank_source_index_lookup_result(
    mut lookup: ClientDbSourceIndexLookupResult,
    query: &str,
) -> ClientDbSourceIndexLookupResult {
    lookup.candidates = reorder_source_index_candidates(
        lookup.candidates,
        query,
        |candidate| candidate.path.clone(),
        |candidate| candidate.query_keys.clone(),
    );
    lookup
}
