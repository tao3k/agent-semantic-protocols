use crate::{
    MemorySearchBackendV1, MemorySearchGenerationV1, MemorySearchIndexV1, MemorySearchItemV1,
};

pub const MEMORY_SEARCH_FIXTURE_SCHEMA_ID: &str = "agent.semantic-protocols.memory-search-fixture";
pub const MEMORY_SEARCH_FIXTURE_SCHEMA_VERSION: &str = "1";

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MemorySearchFixtureV1 {
    pub schema_id: String,
    pub schema_version: String,
    pub generation: MemorySearchGenerationV1,
    pub items: Vec<MemorySearchItemV1>,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct MemorySearchScenarioContractV1 {
    pub schema_id: String,
    pub schema_version: String,
    pub scenario_id: String,
    pub backend: String,
    pub reader_count: usize,
}

impl MemorySearchScenarioContractV1 {
    pub fn from_toml(source: &str) -> Result<Self, String> {
        let contract = toml::from_str::<Self>(source)
            .map_err(|error| format!("invalid memory-search scenario contract: {error}"))?;
        if contract.schema_id != "memory-search-fixture-v1"
            || contract.schema_version != MEMORY_SEARCH_FIXTURE_SCHEMA_VERSION
            || contract.scenario_id.is_empty()
            || contract.backend != "memory"
            || contract.reader_count == 0
        {
            return Err("invalid memory-search scenario evidence".to_string());
        }
        Ok(contract)
    }
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct MemorySearchBenchmarkContractV1 {
    pub max_cold_lookup_micros: u64,
    pub max_warm_lookup_micros: u64,
    pub max_provider_process_count: usize,
    pub max_source_bytes_materialized: usize,
    pub max_db_opens: usize,
    pub max_db_queries: usize,
    pub max_cache_writes: usize,
}

impl MemorySearchBenchmarkContractV1 {
    pub fn from_toml(source: &str) -> Result<Self, String> {
        let contract = toml::from_str::<Self>(source)
            .map_err(|error| format!("invalid memory-search benchmark contract: {error}"))?;
        if contract.max_cold_lookup_micros == 0
            || contract.max_warm_lookup_micros == 0
            || contract.max_warm_lookup_micros > contract.max_cold_lookup_micros
        {
            return Err("invalid memory-search benchmark thresholds".to_string());
        }
        Ok(contract)
    }

    pub fn validate_receipt(
        &self,
        receipt: &crate::memory_search::MemorySearchPerformanceReceiptV1,
    ) -> Result<(), String> {
        let limits = [
            (
                "generation_load_micros",
                receipt.generation_load_micros,
                u128::from(self.max_cold_lookup_micros),
            ),
            (
                "index_lookup_micros",
                receipt.index_lookup_micros,
                u128::from(self.max_warm_lookup_micros),
            ),
            (
                "provider_subprocesses",
                receipt.provider_subprocesses as u128,
                self.max_provider_process_count as u128,
            ),
            (
                "source_bytes_materialized",
                receipt.source_bytes_materialized as u128,
                self.max_source_bytes_materialized as u128,
            ),
            (
                "db_opens",
                receipt.db_opens as u128,
                self.max_db_opens as u128,
            ),
            (
                "db_queries",
                receipt.db_queries as u128,
                self.max_db_queries as u128,
            ),
            (
                "cache_writes",
                receipt.cache_writes as u128,
                self.max_cache_writes as u128,
            ),
        ];
        if let Some((field, observed, maximum)) = limits
            .into_iter()
            .find(|(_, observed, maximum)| observed > maximum)
        {
            return Err(format!(
                "memory-search performance contract exceeded: field={field} observed={observed} maximum={maximum}"
            ));
        }
        Ok(())
    }
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

    pub fn build(self) -> Result<MemorySearchIndexV1, String> {
        if self.schema_id != MEMORY_SEARCH_FIXTURE_SCHEMA_ID
            || self.schema_version != MEMORY_SEARCH_FIXTURE_SCHEMA_VERSION
        {
            return Err("unsupported memory-search fixture contract".to_string());
        }
        MemorySearchIndexV1::build(self.generation, self.items)
    }
}

impl MemorySearchBackendV1 for MemorySearchFixtureV1 {
    fn load_generation(&self) -> Result<MemorySearchIndexV1, String> {
        self.clone().build()
    }
}
