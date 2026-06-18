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
    ClientDbSourceIndexSelector, ClientDbSourceIndexSelectorLookup, ClientDbSourceIndexStats,
};

impl ClientDb {
    /// Replace Rust-owned source index rows for one cache generation.
    pub fn replace_source_index(
        &mut self,
        import: &ClientDbSourceIndexImport,
    ) -> Result<ClientDbSourceIndexStats, String> {
        replace_source_index_rows(self, import)
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
