use agent_semantic_client_core::CacheGenerationId;

use crate::db::ClientDb;

use super::lookup::{lookup_source_index_owners, source_index_stats};
use super::storage::replace_source_index_rows;
use super::types::{
    ClientDbSourceIndexImport, ClientDbSourceIndexLookup, ClientDbSourceIndexOwner,
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

    /// Return source index row counts for one generation.
    pub fn source_index_stats(
        &self,
        generation_id: &CacheGenerationId,
    ) -> Result<ClientDbSourceIndexStats, String> {
        source_index_stats(self, generation_id)
    }

    /// Return Rust-owned source owners matching a broad query from the freshest
    /// matching source index generation.
    pub fn lookup_source_index_owners(
        &self,
        lookup: &ClientDbSourceIndexLookup,
    ) -> Result<Vec<ClientDbSourceIndexOwner>, String> {
        lookup_source_index_owners(self, lookup)
    }
}
