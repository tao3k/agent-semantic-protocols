use crate::memory_search::{
    MemorySearchGenerationV1, MemorySearchIndexV1, MemorySearchItemV1,
    MemorySearchPerformanceReceiptV1,
};
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex, OnceLock};

pub const MEMORY_SEARCH_FIXTURE_SCHEMA_ID: &str = "agent.semantic-protocols.memory-search-fixture";
pub const MEMORY_SEARCH_FIXTURE_SCHEMA_VERSION: &str = "1";

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MemorySearchFixtureV1 {
    pub schema_id: String,
    pub schema_version: String,
    pub generation: MemorySearchGenerationV1,
    pub items: Vec<MemorySearchItemV1>,
}

impl MemorySearchFixtureV1 {
    pub fn new(generation: MemorySearchGenerationV1) -> Self {
        Self {
            schema_id: MEMORY_SEARCH_FIXTURE_SCHEMA_ID.to_owned(),
            schema_version: MEMORY_SEARCH_FIXTURE_SCHEMA_VERSION.to_owned(),
            generation,
            items: Vec::new(),
        }
    }

    pub fn with_item(mut self, item: MemorySearchItemV1) -> Self {
        self.items.push(item);
        self
    }

    pub fn load_generation(&self) -> Result<MemorySearchIndexV1, String> {
        self.build()
    }

    pub fn build(&self) -> Result<MemorySearchIndexV1, String> {
        MemorySearchIndexV1::build(self.generation.clone(), self.items.clone())
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize)]
pub struct MemorySearchScenarioContractV1 {
    pub schema_id: String,
    pub schema_version: String,
    pub backend: String,
    pub reader_count: usize,
}

impl MemorySearchScenarioContractV1 {
    pub fn from_toml(source: &str) -> Result<Self, String> {
        toml::from_str(source)
            .map_err(|error| format!("invalid memory-search scenario contract: {error}"))
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize)]
pub struct MemorySearchBenchmarkContractV1 {
    pub max_cold_lookup_micros: u128,
    pub max_warm_lookup_micros: u128,
    pub max_provider_process_count: usize,
    pub max_source_bytes_materialized: usize,
    pub max_db_opens: usize,
    pub max_db_queries: usize,
    pub max_cache_writes: usize,
}

impl MemorySearchBenchmarkContractV1 {
    pub fn from_toml(source: &str) -> Result<Self, String> {
        toml::from_str(source)
            .map_err(|error| format!("invalid memory-search benchmark contract: {error}"))
    }

    pub fn validate_receipt(
        &self,
        receipt: &MemorySearchPerformanceReceiptV1,
    ) -> Result<(), String> {
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
                "memory-search zero-IO contract violated: providerProcesses={} sourceBytesMaterialized={} dbOpens={} dbQueries={} cacheWrites={}",
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

#[derive(Default)]
pub struct MemorySearchFixtureSlotV1 {
    loaded: OnceLock<Arc<MemorySearchIndexV1>>,
    load_lock: Mutex<()>,
}

impl MemorySearchFixtureSlotV1 {
    pub fn load_once(
        &self,
        fixture: &MemorySearchFixtureV1,
    ) -> Result<Arc<MemorySearchIndexV1>, String> {
        if let Some(index) = self.loaded.get() {
            return Ok(Arc::clone(index));
        }
        let _load_guard = self
            .load_lock
            .lock()
            .map_err(|_| "resident Memory Search load lock poisoned".to_owned())?;
        if let Some(index) = self.loaded.get() {
            return Ok(Arc::clone(index));
        }
        let index = Arc::new(fixture.load_generation()?);
        if self.loaded.set(Arc::clone(&index)).is_err() {
            unreachable!("load lock guarantees a single Memory Search publisher");
        }
        Ok(index)
    }

    pub fn get(&self) -> Option<Arc<MemorySearchIndexV1>> {
        self.loaded.get().map(Arc::clone)
    }
}
