use agent_semantic_client_core::{CacheGenerationId, ClientCacheGeneration};

use crate::db::ClientDb;

use super::lookup::{
    lookup_structural_dependency_usages, lookup_structural_symbols, structural_index_stats,
};
use super::packet::parse_structural_index_packet_import;
use super::storage::{
    apply_structural_index_refresh_rows, plan_structural_index_refresh,
    replace_structural_index_rows,
};
use super::types::{
    ClientDbStructuralDependencyUsage, ClientDbStructuralIndexImport,
    ClientDbStructuralIndexLookup, ClientDbStructuralIndexRefreshPlan,
    ClientDbStructuralIndexStats, ClientDbStructuralSymbol,
};

impl ClientDb {
    /// Import a provider-emitted `semantic-structural-index.v1` packet into
    /// normalized SQLite rows.
    pub fn import_semantic_structural_index_packet(
        &mut self,
        generation: &ClientCacheGeneration,
        packet_bytes: &[u8],
    ) -> Result<ClientDbStructuralIndexStats, String> {
        let import = parse_structural_index_packet_import(generation, packet_bytes)?;
        self.replace_structural_index(&import)
    }

    /// Import a provider-emitted structural index feed through the ASP-owned
    /// incremental refresh path.
    pub fn import_semantic_structural_index_refresh_packet(
        &mut self,
        generation: &ClientCacheGeneration,
        packet_bytes: &[u8],
    ) -> Result<ClientDbStructuralIndexStats, String> {
        let import = parse_structural_index_packet_import(generation, packet_bytes)?;
        self.apply_structural_index_refresh(&import)
    }

    /// Replace structural index rows for one cache generation.
    pub fn replace_structural_index(
        &mut self,
        import: &ClientDbStructuralIndexImport,
    ) -> Result<ClientDbStructuralIndexStats, String> {
        replace_structural_index_rows(self, import)
    }

    /// Plan a structural index refresh from provider file hash evidence.
    pub fn plan_structural_index_refresh(
        &self,
        import: &ClientDbStructuralIndexImport,
    ) -> Result<ClientDbStructuralIndexRefreshPlan, String> {
        plan_structural_index_refresh(self, import)
    }

    /// Apply a structural index refresh by reusing unchanged previous rows.
    pub fn apply_structural_index_refresh(
        &mut self,
        import: &ClientDbStructuralIndexImport,
    ) -> Result<ClientDbStructuralIndexStats, String> {
        apply_structural_index_refresh_rows(self, import)
    }

    /// Return structural index row counts for one generation.
    pub fn structural_index_stats(
        &self,
        generation_id: &CacheGenerationId,
    ) -> Result<ClientDbStructuralIndexStats, String> {
        structural_index_stats(self, generation_id)
    }

    /// Return structural symbols matching a query from freshest matching generations.
    pub fn lookup_structural_symbols(
        &self,
        lookup: &ClientDbStructuralIndexLookup,
    ) -> Result<Vec<ClientDbStructuralSymbol>, String> {
        lookup_structural_symbols(self, lookup)
    }

    /// Return dependency usage rows matching a package or API query.
    pub fn lookup_structural_dependency_usages(
        &self,
        lookup: &ClientDbStructuralIndexLookup,
    ) -> Result<Vec<ClientDbStructuralDependencyUsage>, String> {
        lookup_structural_dependency_usages(self, lookup)
    }
}
