//! Public lookup API for Rust SQL source-index candidates.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use agent_semantic_client_core::{LanguageId, project_client_cache_dir_read_only};
use agent_semantic_client_db::{
    ClientDb, ClientDbEngine, ClientDbSourceIndexLookup, ClientDbSourceIndexQueryKey,
};
use agent_semantic_search::{
    SourceIndexRankCandidate, rank_source_index_candidates, source_index_lookup_terms,
};

use super::model::{SourceIndexCandidate, SourceIndexLookupResult, SourceIndexLookupState};

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

/// Lookup source-index owners from the Rust SQL cache.
pub fn lookup_source_index(
    project_root: &Path,
    query: &str,
    limit: u32,
) -> Result<SourceIndexLookupResult, String> {
    lookup_source_index_for_language(project_root, None, query, limit)
}

/// Lookup source-index owners from the Rust SQL cache for one language scope.
pub fn lookup_source_index_for_language(
    project_root: &Path,
    language_id: Option<&LanguageId>,
    query: &str,
    limit: u32,
) -> Result<SourceIndexLookupResult, String> {
    lookup_source_index_in_cache(SourceIndexLookupRequest {
        cache_project_root: project_root,
        indexed_project_root: project_root,
        language_id,
        query,
        limit,
    })
}

/// Lookup source-index owners from one project's Rust SQL cache for an explicit
/// indexed project root.
pub fn lookup_source_index_in_cache(
    request: SourceIndexLookupRequest<'_>,
) -> Result<SourceIndexLookupResult, String> {
    let cache_root = project_client_cache_dir_read_only(request.cache_project_root)?;
    lookup_source_index_in_client_cache_dir(SourceIndexClientCacheLookupRequest {
        cache_root: &cache_root,
        indexed_project_root: request.indexed_project_root,
        language_id: request.language_id,
        query: request.query,
        limit: request.limit,
    })
}

/// Lookup source-index owners from an already resolved client cache directory.
pub fn lookup_source_index_in_client_cache_dir(
    request: SourceIndexClientCacheLookupRequest<'_>,
) -> Result<SourceIndexLookupResult, String> {
    let db_path = ClientDbEngine::db_path_for_client_dir(request.cache_root);
    let Some(db) = ClientDbEngine::open_read_only_existing_client_dir(request.cache_root)? else {
        return Ok(source_index_lookup_result(
            db_path,
            SourceIndexLookupState::MissingDb,
            Vec::new(),
        ));
    };
    let candidates = lookup_source_index_candidates(
        &db,
        request.indexed_project_root,
        request.language_id,
        request.query,
        request.limit,
    )?;
    let state = if candidates.is_empty() {
        if db.summary()?.source_index_owner_count == 0 {
            SourceIndexLookupState::EmptyIndex
        } else {
            SourceIndexLookupState::Miss
        }
    } else {
        SourceIndexLookupState::Hit
    };
    Ok(source_index_lookup_result(db_path, state, candidates))
}

fn lookup_source_index_candidates(
    db: &ClientDb,
    project_root: &Path,
    language_id: Option<&LanguageId>,
    query: &str,
    limit: u32,
) -> Result<Vec<SourceIndexCandidate>, String> {
    let mut seen = BTreeSet::new();
    let mut candidates = Vec::new();
    for term in source_index_lookup_terms(query) {
        if candidates.len() >= limit as usize {
            break;
        }
        let remaining = limit.saturating_sub(candidates.len() as u32);
        let owners = db.lookup_source_index_owners(&ClientDbSourceIndexLookup {
            project_root: project_root.to_path_buf(),
            language_id: language_id.cloned(),
            query: ClientDbSourceIndexQueryKey::from(term),
            limit: remaining,
        })?;
        append_unique_source_index_candidates(&mut candidates, &mut seen, owners, limit);
    }
    Ok(rank_source_index_lookup_candidates(candidates, query))
}

fn rank_source_index_lookup_candidates(
    candidates: Vec<SourceIndexCandidate>,
    query: &str,
) -> Vec<SourceIndexCandidate> {
    let ranked = rank_source_index_candidates(
        candidates
            .iter()
            .enumerate()
            .map(|(ordinal, candidate)| SourceIndexRankCandidate {
                ordinal,
                path: candidate.path.clone(),
                query_keys: candidate.query_keys.clone(),
            })
            .collect(),
        query,
    );
    let mut candidates = candidates
        .into_iter()
        .map(Some)
        .collect::<Vec<Option<SourceIndexCandidate>>>();
    ranked
        .into_iter()
        .filter_map(|candidate| candidates.get_mut(candidate.ordinal).and_then(Option::take))
        .collect()
}

fn append_unique_source_index_candidates(
    candidates: &mut Vec<SourceIndexCandidate>,
    seen: &mut BTreeSet<String>,
    owners: Vec<impl Into<SourceIndexCandidate>>,
    limit: u32,
) {
    for owner in owners {
        let candidate = owner.into();
        if candidates.len() >= limit as usize {
            break;
        }
        if seen.insert(candidate.path.clone()) {
            candidates.push(candidate);
        }
    }
}

fn source_index_lookup_result(
    db_path: PathBuf,
    state: SourceIndexLookupState,
    candidates: Vec<SourceIndexCandidate>,
) -> SourceIndexLookupResult {
    SourceIndexLookupResult {
        db_path,
        state,
        candidates,
    }
}
