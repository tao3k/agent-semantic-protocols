use std::path::Path;

use crate::db::ClientDb;
use agent_semantic_client_core::{
    CacheGenerationId, ClientCacheFileHash, SemanticSchemaId, SemanticSchemaVersion,
};

use super::lookup::{
    latest_source_index_generation_owners, lookup_source_index_owners,
    lookup_source_index_selectors, source_index_stats,
};
use super::storage::{
    latest_source_index_file_hashes, replace_source_index_rows,
    reusable_source_index_generation_stats,
};
use super::types::{
    ClientDbSourceIndexImport, ClientDbSourceIndexLookup, ClientDbSourceIndexOwner,
    ClientDbSourceIndexRefreshReport, ClientDbSourceIndexRefreshRequest,
    ClientDbSourceIndexScopeFile, ClientDbSourceIndexSelector, ClientDbSourceIndexSelectorLookup,
    ClientDbSourceIndexStats,
};

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

    /// Return Rust-owned source selectors matching a structured lookup from
    /// the freshest matching source index generation.
    pub fn lookup_source_index_selectors(
        &self,
        lookup: &ClientDbSourceIndexSelectorLookup,
    ) -> Result<Vec<ClientDbSourceIndexSelector>, String> {
        lookup_source_index_selectors(self, lookup)
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
