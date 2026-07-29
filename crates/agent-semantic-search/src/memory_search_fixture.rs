use agent_semantic_client_db::{
    ClientDbEngine, ClientDbSourceIndexRefreshReport, ClientDbSourceIndexRefreshRequest,
    ProviderIncrementalScoped, WorkspaceDbRegistry,
};
use serde::Deserialize;

use crate::memory_search::MemorySearchPerformanceReceipt;
use crate::memory_search_resident::MemorySearchResident;
use crate::memory_search_turso::{TursoMemorySearchBackend, TursoMemorySearchBinding};

pub const MEMORY_SEARCH_FIXTURE_SCHEMA_ID: &str =
    "agent.semantic-protocols.memory-search-fixture.v1";
pub const MEMORY_SEARCH_FIXTURE_SCHEMA_VERSION: &str = "1";

/// Production-shaped Turso fixture input.
///
/// The fixture publishes the same source-index generation transaction used by
/// installed ASP, then acquires the same canonical workspace registry entry.
/// It never constructs a Rust-side selector index.
#[derive(Clone, Debug)]
pub struct MemorySearchFixture {
    pub schema_id: String,
    pub schema_version: String,
    pub project_root: std::path::PathBuf,
    pub scope: ProviderIncrementalScoped,
    pub refresh_request: ClientDbSourceIndexRefreshRequest,
    pub binding: TursoMemorySearchBinding,
}

impl MemorySearchFixture {
    pub fn new(
        project_root: std::path::PathBuf,
        scope: ProviderIncrementalScoped,
        refresh_request: ClientDbSourceIndexRefreshRequest,
        binding: TursoMemorySearchBinding,
    ) -> Self {
        Self {
            schema_id: MEMORY_SEARCH_FIXTURE_SCHEMA_ID.to_owned(),
            schema_version: MEMORY_SEARCH_FIXTURE_SCHEMA_VERSION.to_owned(),
            project_root,
            scope,
            refresh_request,
            binding,
        }
    }

    pub async fn publish(
        &self,
        registry: &WorkspaceDbRegistry,
    ) -> Result<(MemorySearchResident, ClientDbSourceIndexRefreshReport), String> {
        if self.schema_id != MEMORY_SEARCH_FIXTURE_SCHEMA_ID
            || self.schema_version != MEMORY_SEARCH_FIXTURE_SCHEMA_VERSION
        {
            return Err("Memory Search fixture schema identity mismatch".to_string());
        }
        let engine = ClientDbEngine::resolve_for_write(&self.project_root)?;
        let refresh = engine
            .refresh_source_index_generation(self.refresh_request.clone())
            .await?;
        let session = registry.acquire(&self.project_root, &self.scope).await?;
        let backend = TursoMemorySearchBackend::new_fixture(
            session,
            self.refresh_request
                .import
                .project_root
                .display()
                .to_string(),
            self.refresh_request.import.schema_id.as_str(),
            self.refresh_request.import.schema_version.as_str(),
            self.binding.clone(),
        )?;
        Ok((MemorySearchResident::new(backend), refresh))
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize)]
pub struct MemorySearchScenarioContract {
    pub schema_id: String,
    pub schema_version: String,
    pub backend: String,
    pub reader_count: usize,
}

impl MemorySearchScenarioContract {
    pub fn from_toml(source: &str) -> Result<Self, String> {
        toml::from_str(source)
            .map_err(|error| format!("invalid memory-search scenario contract: {error}"))
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize)]
pub struct MemorySearchBenchmarkContract {
    pub max_cold_lookup_micros: u128,
    pub max_warm_lookup_micros: u128,
    pub max_provider_process_count: usize,
    pub max_source_bytes_materialized: usize,
    pub max_db_opens: usize,
    pub max_db_queries: usize,
    pub max_cache_writes: usize,
}

impl MemorySearchBenchmarkContract {
    pub fn from_toml(source: &str) -> Result<Self, String> {
        toml::from_str(source)
            .map_err(|error| format!("invalid memory-search benchmark contract: {error}"))
    }

    pub fn validate_receipt(&self, receipt: &MemorySearchPerformanceReceipt) -> Result<(), String> {
        if receipt.generation_load_micros > self.max_cold_lookup_micros {
            return Err(format!(
                "memory-search cold lookup exceeded budget: actualMicros={} budgetMicros={}",
                receipt.generation_load_micros, self.max_cold_lookup_micros
            ));
        }
        if receipt.index_lookup_micros > self.max_warm_lookup_micros {
            return Err(format!(
                "memory-search warm lookup exceeded budget: actualMicros={} budgetMicros={}",
                receipt.index_lookup_micros, self.max_warm_lookup_micros
            ));
        }
        if receipt.provider_subprocesses > self.max_provider_process_count
            || receipt.source_bytes_materialized > self.max_source_bytes_materialized
            || receipt.db_opens > self.max_db_opens
            || receipt.db_queries > self.max_db_queries
            || receipt.cache_writes > self.max_cache_writes
        {
            return Err(format!(
                "memory-search runtime contract violated: providerProcesses={} sourceBytesMaterialized={} dbOpens={} dbQueries={} cacheWrites={}",
                receipt.provider_subprocesses,
                receipt.source_bytes_materialized,
                receipt.db_opens,
                receipt.db_queries,
                receipt.cache_writes
            ));
        }
        Ok(())
    }
}
