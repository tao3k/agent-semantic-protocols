//! DB Engine backend facade, control adapter, and Turso backend boundary.

mod contract;
mod facade;
mod facade_turso_report;
mod search_facade;
mod session_facade;
mod source_index_candidate_selection;
mod source_index_candidate_types;
mod source_index_facade;
mod source_index_query_scoring;
mod turso;
mod turso_artifact;
mod turso_artifact_graph;
mod turso_bootstrap;
mod turso_cache;
pub(crate) mod turso_lock_policy;
mod turso_provider_command;
#[cfg(test)]
#[path = "../../tests/unit/db/engine/turso_schema_lifecycle_private.rs"]
mod turso_schema_lifecycle_tests;
mod turso_search;
mod turso_source_index;
pub(crate) mod turso_statement;
mod turso_syntax;

pub use contract::{ClientDbBackend, ClientDbEngineDurability, ClientDbEngineFeatures};
pub use facade::{
    ClientDbEngine, ClientDbEngineReadSession, ClientDbEngineReport, ClientDbEngineWriteSession,
};
pub use facade::{
    ClientDbEngineSourceIndexReadModelReport, ClientDbEngineStructuralIndexReadModelReport,
};
pub use turso::TURSO_BOOTSTRAP_TABLE;
pub use turso::TursoClientDbEngineReport;
pub use turso_search::{
    TursoClientDbSearchDocument, TursoClientDbSearchHit, TursoClientDbSearchResult,
    TursoClientDbSearchState,
};
