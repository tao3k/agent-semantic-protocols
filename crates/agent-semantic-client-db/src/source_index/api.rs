use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use crate::db::ClientDb;
use agent_semantic_client_core::{
    CacheGenerationId, ClientCacheFileHash, SemanticSchemaId, SemanticSchemaVersion,
    project_client_cache_dir_read_only,
};

use crate::ClientDbEngine;

use super::lookup::{
    latest_source_index_generation_owners, lookup_source_index_owners,
    lookup_source_index_selectors, source_index_stats,
};
use super::storage::{
    latest_source_index_file_hashes, replace_source_index_rows,
    reusable_source_index_generation_stats,
};
use super::types::{
    ClientDbSourceIndexCandidate, ClientDbSourceIndexCandidateLookup,
    ClientDbSourceIndexCandidateLookupResult, ClientDbSourceIndexClientDirLookupRequest,
    ClientDbSourceIndexImport, ClientDbSourceIndexLookup, ClientDbSourceIndexLookupResult,
    ClientDbSourceIndexLookupState, ClientDbSourceIndexOwner,
    ClientDbSourceIndexProjectLookupRequest, ClientDbSourceIndexRefreshReport,
    ClientDbSourceIndexRefreshRequest, ClientDbSourceIndexScopeFile, ClientDbSourceIndexSelector,
    ClientDbSourceIndexSelectorLookup, ClientDbSourceIndexStats,
};

/// Lookup source-index candidates from a project's resolved client DB.
pub fn lookup_source_index_from_project(
    request: ClientDbSourceIndexProjectLookupRequest<'_>,
) -> Result<ClientDbSourceIndexLookupResult, String> {
    let client_dir = project_client_cache_dir_read_only(request.cache_project_root)?;
    lookup_source_index_from_client_dir(ClientDbSourceIndexClientDirLookupRequest {
        client_dir: &client_dir,
        indexed_project_root: request.indexed_project_root,
        language_id: request.language_id,
        query_keys: request.query_keys,
        limit: request.limit,
    })
}

/// Lookup source-index candidates from an already resolved client DB directory.
pub fn lookup_source_index_from_client_dir(
    request: ClientDbSourceIndexClientDirLookupRequest<'_>,
) -> Result<ClientDbSourceIndexLookupResult, String> {
    let db_path = ClientDbEngine::db_path_for_client_dir(request.client_dir);
    let Some(db) = ClientDbEngine::open_read_only_existing_client_dir(request.client_dir)? else {
        return Ok(source_index_lookup_result(
            db_path,
            ClientDbSourceIndexLookupState::MissingDb,
            Vec::new(),
        ));
    };
    let lookup = db.lookup_source_index_candidates(&ClientDbSourceIndexCandidateLookup {
        project_root: request.indexed_project_root.to_path_buf(),
        language_id: request.language_id.cloned(),
        query_keys: request.query_keys,
        limit: request.limit,
    })?;
    Ok(source_index_lookup_result(
        db_path,
        lookup.state,
        lookup.candidates,
    ))
}

impl ClientDb {
    /// Replace Rust-owned source index rows for one cache generation.
    pub fn replace_source_index(
        &mut self,
        import: &ClientDbSourceIndexImport,
    ) -> Result<ClientDbSourceIndexStats, String> {
        replace_source_index_rows(self, import)
    }

    /// Apply a source-index import and return the DB-owned refresh report.
    pub fn refresh_source_index_import(
        &mut self,
        request: ClientDbSourceIndexRefreshRequest,
    ) -> Result<ClientDbSourceIndexRefreshReport, String> {
        let requested_generation_id = request.import.generation_id.clone();
        let stats = replace_source_index_rows(self, &request.import)?;
        Ok(source_index_refresh_report(
            stats,
            request.file_count,
            requested_generation_id,
        ))
    }

    /// Return source index row counts for one generation.
    pub fn source_index_stats(
        &self,
        generation_id: &CacheGenerationId,
    ) -> Result<ClientDbSourceIndexStats, String> {
        source_index_stats(self, generation_id)
    }

    /// Return reusable row counts when the latest source index evidence is unchanged.
    pub fn reusable_source_index_generation(
        &self,
        project_root: &Path,
        schema_id: &SemanticSchemaId,
        schema_version: &SemanticSchemaVersion,
        file_hashes: &[ClientCacheFileHash],
    ) -> Result<Option<ClientDbSourceIndexStats>, String> {
        reusable_source_index_generation_stats(
            self,
            project_root,
            schema_id,
            schema_version,
            file_hashes,
        )
    }

    /// Return file hash evidence from the latest matching source index generation.
    pub fn latest_source_index_file_hashes(
        &self,
        project_root: &Path,
        schema_id: &SemanticSchemaId,
        schema_version: &SemanticSchemaVersion,
    ) -> Result<Option<Vec<ClientCacheFileHash>>, String> {
        latest_source_index_file_hashes(self, project_root, schema_id, schema_version)
    }

    /// Return source owners from the latest matching project generation.
    pub fn latest_source_index_generation_owners(
        &self,
        project_root: &Path,
        schema_id: &SemanticSchemaId,
        schema_version: &SemanticSchemaVersion,
    ) -> Result<Vec<ClientDbSourceIndexOwner>, String> {
        latest_source_index_generation_owners(self, project_root, schema_id, schema_version)
    }

    /// Return file-scoped source-index inputs reconstructed from the latest
    /// matching project generation.
    pub fn latest_source_index_scope_files(
        &self,
        project_root: &Path,
        schema_id: &SemanticSchemaId,
        schema_version: &SemanticSchemaVersion,
    ) -> Result<Option<Vec<ClientDbSourceIndexScopeFile>>, String> {
        let owners =
            latest_source_index_generation_owners(self, project_root, schema_id, schema_version)?;
        if owners.is_empty() {
            return Ok(None);
        }
        let mut files = Vec::new();
        for owner in owners {
            if owner.source_kind.as_str() != "file" {
                continue;
            }
            let (Some(language_id), Some(provider_id)) = (owner.language_id, owner.provider_id)
            else {
                return Ok(None);
            };
            files.push(ClientDbSourceIndexScopeFile {
                path: project_root.join(owner.owner_path.as_str()),
                language_id,
                provider_id,
            });
        }
        if files.is_empty() {
            Ok(None)
        } else {
            Ok(Some(files))
        }
    }

    /// Return Rust-owned source owners matching a broad query from the freshest
    /// matching source index generation.
    pub fn lookup_source_index_owners(
        &self,
        lookup: &ClientDbSourceIndexLookup,
    ) -> Result<Vec<ClientDbSourceIndexOwner>, String> {
        lookup_source_index_owners(self, lookup)
    }

    /// Return deduplicated source-index candidates for a multi-key lookup and
    /// classify whether the latest DB generation was hit, missed, or empty.
    pub fn lookup_source_index_candidates(
        &self,
        lookup: &ClientDbSourceIndexCandidateLookup,
    ) -> Result<ClientDbSourceIndexCandidateLookupResult, String> {
        let candidates = lookup_source_index_candidates(self, lookup)?;
        let state = if candidates.is_empty() {
            if self.summary()?.source_index_owner_count == 0 {
                ClientDbSourceIndexLookupState::EmptyIndex
            } else {
                ClientDbSourceIndexLookupState::Miss
            }
        } else {
            ClientDbSourceIndexLookupState::Hit
        };
        Ok(ClientDbSourceIndexCandidateLookupResult { state, candidates })
    }

    /// Return Rust-owned source selectors matching a structured lookup from
    /// the freshest matching source index generation.
    pub fn lookup_source_index_selectors(
        &self,
        lookup: &ClientDbSourceIndexSelectorLookup,
    ) -> Result<Vec<ClientDbSourceIndexSelector>, String> {
        lookup_source_index_selectors(self, lookup)
    }
}

fn lookup_source_index_candidates(
    db: &ClientDb,
    lookup: &ClientDbSourceIndexCandidateLookup,
) -> Result<Vec<ClientDbSourceIndexCandidate>, String> {
    if lookup.limit == 0 || lookup.query_keys.is_empty() {
        return Ok(Vec::new());
    }
    let mut seen = BTreeSet::new();
    let mut candidates = Vec::new();
    for query in &lookup.query_keys {
        if candidates.len() >= lookup.limit as usize {
            break;
        }
        let remaining = lookup.limit.saturating_sub(candidates.len() as u32);
        let owners = lookup_source_index_owners(
            db,
            &ClientDbSourceIndexLookup {
                project_root: lookup.project_root.clone(),
                language_id: lookup.language_id.clone(),
                query: query.clone(),
                limit: remaining,
            },
        )?;
        append_unique_source_index_candidates(&mut candidates, &mut seen, owners, lookup.limit);
    }
    Ok(candidates)
}

fn append_unique_source_index_candidates(
    candidates: &mut Vec<ClientDbSourceIndexCandidate>,
    seen: &mut BTreeSet<String>,
    owners: Vec<ClientDbSourceIndexOwner>,
    limit: u32,
) {
    for owner in owners {
        let candidate = ClientDbSourceIndexCandidate::from(owner);
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
    state: ClientDbSourceIndexLookupState,
    candidates: Vec<ClientDbSourceIndexCandidate>,
) -> ClientDbSourceIndexLookupResult {
    ClientDbSourceIndexLookupResult {
        db_path,
        state,
        candidates,
    }
}

fn source_index_refresh_report(
    stats: ClientDbSourceIndexStats,
    file_count: u32,
    requested_generation_id: CacheGenerationId,
) -> ClientDbSourceIndexRefreshReport {
    ClientDbSourceIndexRefreshReport {
        reused_generation: stats.generation_id != requested_generation_id,
        generation_id: stats.generation_id,
        file_count,
        owner_count: stats.owner_count,
        selector_count: stats.selector_count,
    }
}
