//! SQLite-backed cache database surface for `agent-semantic-client`.

pub mod db;

pub use agent_semantic_client_core::ClientDbStatus;
pub use db::{
    AGENT_SEMANTIC_CLIENT_DB_FILE, AGENT_SEMANTIC_CLIENT_DB_SCHEMA_VERSION, ClientDb,
    ClientDbGenerationHit, ClientDbGenerationLookup, ClientDbReport, ClientDbSummary,
    ClientDbSyntaxCaptureReplay, ClientDbSyntaxQueryInputKind, ClientDbSyntaxQueryLookup,
    ClientDbSyntaxQueryReplay,
};
