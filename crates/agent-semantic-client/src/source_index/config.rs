//! Static limits and filename catalogs for Rust SQL source indexing.

pub(super) use agent_semantic_client_db::{
    CLIENT_DB_SOURCE_INDEX_PROVIDER_ID as SOURCE_INDEX_PROVIDER_ID,
    CLIENT_DB_SOURCE_INDEX_SCHEMA_ID as SOURCE_INDEX_SCHEMA_ID,
    CLIENT_DB_SOURCE_INDEX_SCHEMA_VERSION as SOURCE_INDEX_SCHEMA_VERSION,
};
pub(super) const SOURCE_INDEX_FILE_LIMIT: usize = 4096;
pub(super) const SOURCE_INDEX_FILE_BYTES_LIMIT: u64 = 1_048_576;
