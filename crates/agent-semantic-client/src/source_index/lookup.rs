//! Public lookup API for Rust SQL source-index candidates.

use std::cmp::Reverse;
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use agent_semantic_client_core::{LanguageId, project_client_cache_dir_read_only};
use agent_semantic_client_db::{
    ClientDb, ClientDbEngine, ClientDbSourceIndexLookup, ClientDbSourceIndexOwner,
    ClientDbSourceIndexQueryKey,
};

use super::model::{SourceIndexCandidate, SourceIndexLookupResult, SourceIndexLookupState};
use super::text::lookup_terms;

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
    lookup_source_index_in_cache(project_root, project_root, language_id, query, limit)
}

/// Lookup source-index owners from one project's Rust SQL cache for an explicit
/// indexed project root.
pub fn lookup_source_index_in_cache(
    cache_project_root: &Path,
    indexed_project_root: &Path,
    language_id: Option<&LanguageId>,
    query: &str,
    limit: u32,
) -> Result<SourceIndexLookupResult, String> {
    let cache_root = project_client_cache_dir_read_only(cache_project_root)?;
    lookup_source_index_in_client_cache_dir(
        &cache_root,
        indexed_project_root,
        language_id,
        query,
        limit,
    )
}

/// Lookup source-index owners from an already resolved client cache directory.
pub fn lookup_source_index_in_client_cache_dir(
    cache_root: &Path,
    indexed_project_root: &Path,
    language_id: Option<&LanguageId>,
    query: &str,
    limit: u32,
) -> Result<SourceIndexLookupResult, String> {
    let db_path = ClientDbEngine::sqlite_path_for_client_dir(cache_root);
    let Some(db) = ClientDb::open_read_only_existing(&db_path)? else {
        return Ok(source_index_lookup_result(
            db_path,
            SourceIndexLookupState::MissingDb,
            Vec::new(),
        ));
    };
    let candidates =
        lookup_source_index_candidates(&db, indexed_project_root, language_id, query, limit)?;
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
    for term in lookup_terms(query) {
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
    Ok(rank_source_index_candidates(candidates, query))
}

type SourceIndexCandidateSortKey = (Reverse<usize>, usize);

fn rank_source_index_candidates(
    candidates: Vec<SourceIndexCandidate>,
    query: &str,
) -> Vec<SourceIndexCandidate> {
    let terms = lookup_terms(query);
    let mut indexed = candidates.into_iter().enumerate().collect::<Vec<_>>();
    indexed.sort_by_key(|(index, candidate)| {
        source_index_candidate_sort_key(candidate, terms.as_slice(), *index)
    });
    indexed
        .into_iter()
        .map(|(_index, candidate)| candidate)
        .collect()
}

fn source_index_candidate_sort_key(
    candidate: &SourceIndexCandidate,
    terms: &[String],
    index: usize,
) -> SourceIndexCandidateSortKey {
    (
        Reverse(source_index_candidate_query_axis_coverage(candidate, terms)),
        index,
    )
}

fn source_index_candidate_query_axis_coverage(
    candidate: &SourceIndexCandidate,
    terms: &[String],
) -> usize {
    let normalized_path = candidate.path.to_ascii_lowercase();
    terms
        .iter()
        .filter(|term| {
            !term.is_empty()
                && (normalized_path.contains(term.as_str())
                    || candidate
                        .query_keys
                        .iter()
                        .any(|key| key.contains(term.as_str())))
        })
        .count()
}

fn append_unique_source_index_candidates(
    candidates: &mut Vec<SourceIndexCandidate>,
    seen: &mut BTreeSet<String>,
    owners: Vec<ClientDbSourceIndexOwner>,
    limit: u32,
) {
    for owner in owners {
        if candidates.len() >= limit as usize {
            break;
        }
        if seen.insert(owner.owner_path.as_str().to_string()) {
            candidates.push(source_index_candidate(owner));
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

fn source_index_candidate(owner: ClientDbSourceIndexOwner) -> SourceIndexCandidate {
    SourceIndexCandidate {
        path: owner.owner_path.as_str().to_string(),
        language_id: owner.language_id,
        provider_id: owner.provider_id,
        source_kind: owner.source_kind.into(),
        line_count: owner.line_count,
        query_keys: owner
            .query_keys
            .into_iter()
            .map(|key| key.as_str().to_string())
            .collect(),
    }
}
