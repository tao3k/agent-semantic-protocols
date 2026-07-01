use std::path::Path;

use agent_semantic_client_core::state_core::CLIENT_DB_FILE;

use crate::db::{AGENT_SEMANTIC_CLIENT_DB_SCHEMA_VERSION, ClientDb, ClientDbReport};

use super::contract::{
    ClientDbBackend, ClientDbEngineBackend, ClientDbEngineDurability, ClientDbEngineFeatures,
};

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(super) struct SqliteClientDbEngineBackend;

impl ClientDbEngineBackend for SqliteClientDbEngineBackend {
    type Connection = ClientDb;
    type Report = ClientDbReport;

    fn backend(&self) -> ClientDbBackend {
        ClientDbBackend::SqliteV1
    }

    fn db_file_name(&self) -> &'static str {
        CLIENT_DB_FILE
    }

    fn schema_version(&self) -> i64 {
        AGENT_SEMANTIC_CLIENT_DB_SCHEMA_VERSION
    }

    fn durability(&self) -> ClientDbEngineDurability {
        ClientDbEngineDurability::SqliteFile
    }

    fn features(&self) -> ClientDbEngineFeatures {
        ClientDbEngineFeatures::default()
    }

    fn open_or_create(&self, db_path: &Path) -> Result<Self::Connection, String> {
        ClientDb::open_or_create(db_path)
    }

    fn open_read_only_existing(&self, db_path: &Path) -> Result<Option<Self::Connection>, String> {
        ClientDb::open_read_only_existing(db_path)
    }

    fn inspect(&self, db_path: &Path) -> Self::Report {
        ClientDb::inspect(db_path)
    }
}
