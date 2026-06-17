//! Static limits and filename catalogs for Rust SQL source indexing.

pub(super) const SOURCE_INDEX_SCHEMA_ID: &str = "agent.semantic-protocols.semantic-source-index";
pub(super) const SOURCE_INDEX_SCHEMA_VERSION: &str = "1";
pub(super) const SOURCE_INDEX_PROVIDER_ID: &str = "rust-sql-source-index";
pub(super) const SOURCE_INDEX_QUERY_KEY_LIMIT: usize = 128;
pub(super) const SOURCE_INDEX_FILE_LIMIT: usize = 4096;
pub(super) const SOURCE_INDEX_FILE_BYTES_LIMIT: u64 = 1_048_576;
