//! DB Engine backend facade, control adapter, and Turso backend boundary.

mod contract;
mod facade;
mod turso;
mod turso_artifact;
mod turso_artifact_graph;
mod turso_bootstrap;
mod turso_cache;
mod turso_evidence_graph;
pub(crate) mod turso_lock_policy;
mod turso_operation_lock;
mod turso_provider_command;
mod turso_route_receipt;
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
pub use turso::TursoClientDbEngineReport;
pub use turso::{
    TURSO_BOOTSTRAP_TABLE, TURSO_OVERLAY_DOCUMENT_TABLE, TURSO_ROUTE_RECEIPT_TABLE,
    TURSO_SEARCH_DOCUMENT_TABLE,
};
pub use turso_evidence_graph::{
    TURSO_EDGE_TABLE, TURSO_ENTITY_TABLE, TursoClientDbEvidenceGraphPersistReport,
    TursoClientDbGraphEdge, TursoClientDbGraphEntity, list_turso_graph_edges,
    list_turso_graph_entities, persist_turso_evidence_graph, upsert_turso_graph_edge,
    upsert_turso_graph_entity,
};
pub use turso_route_receipt::{
    TursoClientDbRouteReceipt, list_turso_route_receipts, upsert_turso_route_receipt,
};
pub use turso_search::{
    TursoClientDbOverlayDocument, TursoClientDbSearchDocument, TursoClientDbSearchHit,
};
