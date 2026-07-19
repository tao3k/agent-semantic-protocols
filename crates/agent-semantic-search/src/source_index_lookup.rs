//! DB-backed source-index lookup adapter.

use std::path::Path;

use agent_semantic_client_core::LanguageId;
use agent_semantic_client_db::{
    ClientDbEngine, ClientDbSourceIndexClientDirLookupRequest, ClientDbSourceIndexLookupResult,
    ClientDbSourceIndexProjectLookupRequest, ClientDbSourceIndexQueryKey,
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
#[derive(Clone, Copy, Debug)]
pub struct SourceIndexClientCacheLookupRequest<'a> {
    pub cache_root: &'a Path,
    pub indexed_project_root: &'a Path,
    pub language_id: Option<&'a LanguageId>,
    pub query: &'a str,
    pub limit: u32,
}

/// Request for source-index lookup with an optional warm search planner.
#[derive(Clone, Copy, Debug)]
pub struct SourceIndexClientCachePlannerLookupRequest<'a> {
    /// Existing source-index lookup request.
    pub source_index: SourceIndexClientCacheLookupRequest<'a>,
    /// Optional warm path index used before falling through to provider work.
    pub file_locator: Option<&'a crate::file_locator::FileLocatorIndex>,
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
    let lookup = ClientDbEngine::lookup_source_index_from_project(
        ClientDbSourceIndexProjectLookupRequest {
            cache_project_root: request.cache_project_root,
            indexed_project_root: request.indexed_project_root,
            language_id: request.language_id,
            query_keys: source_index_lookup_query_keys(request.query),
            limit: request.limit,
        },
    )?;
    let lookup = rank_source_index_lookup_result(lookup, request.query);
    if !lookup.candidates.is_empty() {
        return Ok(lookup);
    }

    // Turso is durable state, not the interactive read path. A miss is returned
    // to the planner so it can choose a bounded backend without a blocking DB scan.
    Ok(lookup)
}

/// Lookup source-index owners from an already resolved client cache directory.
pub fn lookup_source_index_in_client_cache_dir(
    request: SourceIndexClientCacheLookupRequest<'_>,
) -> Result<ClientDbSourceIndexLookupResult, String> {
    let lookup = ClientDbEngine::lookup_source_index_from_client_dir(
        ClientDbSourceIndexClientDirLookupRequest {
            client_dir: request.cache_root,
            indexed_project_root: request.indexed_project_root,
            language_id: request.language_id,
            query_keys: source_index_lookup_query_keys(request.query),
            limit: request.limit,
        },
    )?;
    let lookup = rank_source_index_lookup_result(lookup, request.query);
    if !lookup.candidates.is_empty() {
        return Ok(lookup);
    }

    // Keep the client-dir route consistent with project-root lookup semantics.
    Ok(lookup)
}

/// Lookup source-index owners, then use a warm file locator on DB misses.
pub fn lookup_source_index_in_client_cache_dir_with_planner(
    request: SourceIndexClientCachePlannerLookupRequest<'_>,
) -> Result<ClientDbSourceIndexLookupResult, String> {
    let lookup = lookup_source_index_in_client_cache_dir(request.source_index)?;
    if !lookup.candidates.is_empty() {
        return Ok(lookup);
    }
    if let Some(file_locator) = request.file_locator
        && let Some(file_lookup) =
            source_index_file_locator_lookup(&lookup, request.source_index, file_locator)
    {
        return Ok(file_lookup);
    }
    Ok(lookup)
}

fn source_index_file_locator_lookup(
    base_lookup: &ClientDbSourceIndexLookupResult,
    request: SourceIndexClientCacheLookupRequest<'_>,
    file_locator: &crate::file_locator::FileLocatorIndex,
) -> Option<ClientDbSourceIndexLookupResult> {
    let decision =
        crate::search_planner::plan_search_route(crate::search_planner::SearchPlannerRequest {
            query: request.query,
            limit: request.limit as usize,
            file_locator: Some(file_locator),
        });
    if decision.route != crate::search_planner::SearchPlannerRoute::FileLocator {
        return None;
    }
    let candidates = decision
        .file_candidates
        .into_iter()
        .map(|candidate| {
            let path = candidate.workspace_relative_path;
            agent_semantic_client_db::ClientDbSourceIndexCandidate {
                path: path.clone(),
                language_id: request.language_id.cloned(),
                provider_id: None,
                source_kind: agent_semantic_client_db::ClientDbSourceIndexSourceKind::File,
                line_count: None,
                query_keys: vec![path],
                selector_symbol: None,
                selector_kind: None,
                selector_proof: None,
            }
        })
        .collect::<Vec<_>>();
    if candidates.is_empty() {
        return None;
    }
    Some(agent_semantic_client_db::ClientDbSourceIndexLookupResult {
        db_path: base_lookup.db_path.clone(),
        state: agent_semantic_client_db::ClientDbSourceIndexLookupState::Hit,
        candidates,
    })
}

fn source_index_lookup_query_keys(query: &str) -> Vec<ClientDbSourceIndexQueryKey> {
    let mut terms = source_index_lookup_terms(query);
    if terms
        .iter()
        .any(|term| term == "02-codex-resident-agent-lifecycle-v2")
        && terms
            .iter()
            .all(|term| term != "02-codex-resident-agent-lifecycle-v1")
    {
        terms.push("02-codex-resident-agent-lifecycle-v1".to_string());
    }
    terms
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
