//! Public value types for `semantic-structural-index` Turso rows.

use std::path::PathBuf;

use agent_semantic_client_core::{
    CacheArtifactId, CacheExportMethod, CacheGenerationId, ClientCacheFileHash, LanguageId,
    ProviderId, SemanticSchemaId, SemanticSchemaVersion,
};

macro_rules! structural_value_type {
    ($(#[$meta:meta])* $name:ident) => {
        $(#[$meta])*
        #[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
        pub struct $name(String);

        impl $name {
            /// Create a structural index scalar.
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

structural_value_type!(
    /// Project-relative path retained by the structural index.
    ClientDbStructuralPath
);
structural_value_type!(
    /// Parser-owned structural name such as a symbol or package name.
    ClientDbStructuralName
);
structural_value_type!(
    /// Schema-owned structural kind or visibility value.
    ClientDbStructuralKind
);
structural_value_type!(
    /// Provider authority or source channel for a structural row.
    ClientDbStructuralSource
);
structural_value_type!(
    /// Query key used for index-first recall.
    ClientDbStructuralQueryKey
);
structural_value_type!(
    /// Parser-owned source locator retained for last-mile query routing.
    ClientDbStructuralLocator
);
structural_value_type!(
    /// Manifest or lockfile freshness hash.
    ClientDbStructuralHash
);

/// One provider-owned structural index generation imported into the client DB.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClientDbStructuralIndexImport {
    pub generation_id: CacheGenerationId,
    pub language_id: LanguageId,
    pub provider_id: ProviderId,
    pub provider_version: Option<ClientDbStructuralName>,
    pub export_method: Option<CacheExportMethod>,
    pub project_root: PathBuf,
    pub package_root: Option<ClientDbStructuralPath>,
    pub schema_id: SemanticSchemaId,
    pub schema_version: SemanticSchemaVersion,
    pub source_artifact_id: Option<CacheArtifactId>,
    pub file_hashes: Vec<ClientCacheFileHash>,
    pub owners: Vec<ClientDbStructuralOwner>,
    pub symbols: Vec<ClientDbStructuralSymbol>,
    pub dependency_usages: Vec<ClientDbStructuralDependencyUsage>,
}

/// ASP-owned refresh plan derived from provider file hash evidence.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClientDbStructuralIndexRefreshPlan {
    pub unchanged_paths: Vec<ClientDbStructuralPath>,
    pub changed_paths: Vec<ClientDbStructuralPath>,
    pub deleted_paths: Vec<ClientDbStructuralPath>,
}

/// Parser-owned file/owner row retained for index-first cache recall.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClientDbStructuralOwner {
    pub owner_path: ClientDbStructuralPath,
    pub owner_kind: ClientDbStructuralKind,
    pub source_authority: ClientDbStructuralSource,
    pub start_line: Option<u32>,
    pub end_line: Option<u32>,
    pub query_keys: Vec<ClientDbStructuralQueryKey>,
}

/// Parser-owned symbol row retained for index-first cache recall.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClientDbStructuralSymbol {
    pub owner_path: ClientDbStructuralPath,
    pub name: ClientDbStructuralName,
    pub kind: ClientDbStructuralKind,
    pub visibility: Option<ClientDbStructuralKind>,
    pub source_locator: Option<ClientDbStructuralLocator>,
    pub query_keys: Vec<ClientDbStructuralQueryKey>,
}

/// Provider-owned dependency API usage row retained for stable dependency recall.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClientDbStructuralDependencyUsage {
    pub owner_path: ClientDbStructuralPath,
    pub package_name: ClientDbStructuralName,
    pub package_version: Option<ClientDbStructuralName>,
    pub api_name: Option<ClientDbStructuralName>,
    pub import_path: Option<ClientDbStructuralPath>,
    pub manifest_path: Option<ClientDbStructuralPath>,
    pub lockfile_hash: Option<ClientDbStructuralHash>,
    pub source: ClientDbStructuralSource,
    pub source_locator: Option<ClientDbStructuralLocator>,
    pub query_keys: Vec<ClientDbStructuralQueryKey>,
}

/// Aggregate row counts for one structural index generation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClientDbStructuralIndexStats {
    pub generation_id: CacheGenerationId,
    pub owner_count: u32,
    pub symbol_count: u32,
    pub dependency_usage_count: u32,
}

/// Lookup request for structural index rows scoped to one provider project.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClientDbStructuralIndexLookup {
    pub language_id: LanguageId,
    pub provider_id: ProviderId,
    pub project_root: PathBuf,
    pub query: ClientDbStructuralQueryKey,
    pub limit: u32,
}
