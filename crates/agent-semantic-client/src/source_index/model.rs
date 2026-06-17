//! Public data model for Rust SQL source-index refresh and lookup receipts.

use std::path::PathBuf;

use agent_semantic_client_core::{CacheGenerationId, LanguageId, ProviderId};
use agent_semantic_client_db::ClientDbSourceIndexSource;

/// Result of refreshing the Rust SQL source index.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceIndexRefreshReport {
    pub db_path: PathBuf,
    pub generation_id: CacheGenerationId,
    pub file_count: u32,
    pub owner_count: u32,
    pub selector_count: u32,
}

/// Source-index lookup state for agent-facing search fallbacks.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SourceIndexLookupState {
    MissingDb,
    EmptyIndex,
    Hit,
    Miss,
}

impl SourceIndexLookupState {
    #[must_use]
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::MissingDb => "missing-db",
            Self::EmptyIndex => "empty-index",
            Self::Hit => "hit",
            Self::Miss => "miss",
        }
    }
}

/// Agent-facing source-index candidate row.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceIndexCandidate {
    pub path: String,
    pub language_id: Option<LanguageId>,
    pub provider_id: Option<ProviderId>,
    pub source_kind: SourceIndexSourceKind,
    pub line_count: Option<u32>,
    pub query_keys: Vec<String>,
}

/// Typed source category for source-index candidate rows.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SourceIndexSourceKind {
    File,
    Other(String),
}

impl SourceIndexSourceKind {
    #[must_use]
    pub fn as_str(&self) -> &str {
        match self {
            Self::File => "file",
            Self::Other(value) => value.as_str(),
        }
    }
}

impl From<ClientDbSourceIndexSource> for SourceIndexSourceKind {
    fn from(value: ClientDbSourceIndexSource) -> Self {
        match value.as_str() {
            "file" => Self::File,
            other => Self::Other(other.to_string()),
        }
    }
}

/// Lookup result from the Rust SQL source index.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceIndexLookupResult {
    pub db_path: PathBuf,
    pub state: SourceIndexLookupState,
    pub candidates: Vec<SourceIndexCandidate>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct SourceIndexScopeFile {
    pub(super) path: PathBuf,
    pub(super) language_id: LanguageId,
    pub(super) provider_id: ProviderId,
}
