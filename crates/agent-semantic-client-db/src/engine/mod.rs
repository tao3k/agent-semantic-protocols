mod facade;
mod sqlite;
mod turso;

pub use facade::{
    ClientDbBackend, ClientDbEngine, ClientDbEngineDurability, ClientDbEngineFeatures,
    ClientDbEngineReport,
};
pub use turso::TursoClientDbEngineReport;
#[cfg(feature = "turso-backend")]
pub use turso::{
    TURSO_BOOTSTRAP_TABLE, TURSO_ENTITY_TABLE, TURSO_OVERLAY_DOCUMENT_TABLE,
    TURSO_SEARCH_DOCUMENT_TABLE, TursoClientDbOverlayDocument, TursoClientDbSearchDocument,
    TursoClientDbSearchHit, bootstrap_turso_client_db, search_turso_documents,
    upsert_turso_overlay_document, upsert_turso_search_document,
};
