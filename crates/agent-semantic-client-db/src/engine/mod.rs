//! DB Engine backend facade, control adapter, and Turso backend boundary.

mod contract;
mod facade;
mod sqlite;
mod turso;
#[cfg(feature = "turso-backend")]
mod turso_route_receipt;
#[cfg(feature = "turso-backend")]
mod turso_search;

pub use contract::{ClientDbBackend, ClientDbEngineDurability, ClientDbEngineFeatures};
pub use facade::{ClientDbEngine, ClientDbEngineReport};
#[cfg(feature = "turso-backend")]
pub use facade::{
    ClientDbEngineSourceIndexReadModelReport, ClientDbEngineStructuralIndexReadModelReport,
};
pub use turso::TursoClientDbEngineReport;
#[cfg(feature = "turso-backend")]
pub use turso::{
    TURSO_BOOTSTRAP_TABLE, TURSO_EDGE_TABLE, TURSO_ENTITY_TABLE, TURSO_OVERLAY_DOCUMENT_TABLE,
    TURSO_ROUTE_RECEIPT_TABLE, TURSO_SEARCH_DOCUMENT_TABLE,
    TursoClientDbEvidenceGraphPersistReport, TursoClientDbGraphEdge, TursoClientDbGraphEntity,
    list_turso_graph_edges, list_turso_graph_entities, persist_turso_evidence_graph,
    upsert_turso_graph_edge, upsert_turso_graph_entity,
};
#[cfg(feature = "turso-backend")]
pub use turso_route_receipt::{
    TursoClientDbRouteReceipt, list_turso_route_receipts, upsert_turso_route_receipt,
};
#[cfg(feature = "turso-backend")]
pub use turso_search::{
    TursoClientDbOverlayDocument, TursoClientDbSearchDocument, TursoClientDbSearchHit,
};
