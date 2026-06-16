//! Public value types for Rust-owned SQL source index rows.

use std::path::PathBuf;

use agent_semantic_client_core::{
    CacheGenerationId, ClientCacheFileHash, LanguageId, ProviderId, SemanticSchemaId,
    SemanticSchemaVersion,
};

macro_rules! source_index_value_type {
    ($(#[$meta:meta])* $name:ident) => {
        $(#[$meta])*
        #[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
        pub struct $name(String);

        impl $name {
            /// Create a source index scalar.
            #[must_use]
            pub fn new(value: impl Into<String>) -> Self {
                Self(value.into())
            }

            /// Return the stored scalar text.
            #[must_use]
            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl From<&str> for $name {
            fn from(value: &str) -> Self {
                Self(value.to_string())
            }
        }

        impl From<String> for $name {
            fn from(value: String) -> Self {
                Self(value)
            }
        }
    };
}

source_index_value_type!(
    /// Project-relative path retained by the Rust SQL source index.
    ClientDbSourceIndexPath
);
source_index_value_type!(
    /// Query key used for index-first owner recall.
    ClientDbSourceIndexQueryKey
);
source_index_value_type!(
    /// Source authority for a selector or owner row.
    ClientDbSourceIndexSource
);

/// One Rust-owned source index generation imported into the client DB.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClientDbSourceIndexImport {
    pub generation_id: CacheGenerationId,
    pub project_root: PathBuf,
    pub schema_id: SemanticSchemaId,
    pub schema_version: SemanticSchemaVersion,
    pub file_hashes: Vec<ClientCacheFileHash>,
    pub owners: Vec<ClientDbSourceIndexOwner>,
    pub selectors: Vec<ClientDbSourceIndexSelector>,
}

/// Rust-owned owner row retained for index-first broad search.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClientDbSourceIndexOwner {
    pub owner_path: ClientDbSourceIndexPath,
    pub language_id: Option<LanguageId>,
    pub provider_id: Option<ProviderId>,
    pub source_kind: ClientDbSourceIndexSource,
    pub line_count: Option<u32>,
    pub query_keys: Vec<ClientDbSourceIndexQueryKey>,
}

/// Rust-owned selector row retained for exact owner-local expansion.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClientDbSourceIndexSelector {
    pub owner_path: ClientDbSourceIndexPath,
    pub selector_id: String,
    pub symbol: Option<String>,
    pub kind: Option<String>,
    pub start_line: u32,
    pub end_line: u32,
    pub source: ClientDbSourceIndexSource,
    pub query_keys: Vec<ClientDbSourceIndexQueryKey>,
}

/// Aggregate row counts for one source index generation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClientDbSourceIndexStats {
    pub generation_id: CacheGenerationId,
    pub owner_count: u32,
    pub selector_count: u32,
}

/// Lookup request for Rust-owned source index rows.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClientDbSourceIndexLookup {
    pub project_root: PathBuf,
    pub language_id: Option<LanguageId>,
    pub query: ClientDbSourceIndexQueryKey,
    pub limit: u32,
}
