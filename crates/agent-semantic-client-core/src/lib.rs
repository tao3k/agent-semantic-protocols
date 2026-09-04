#![deny(dead_code)]

//! Core contracts for the server-first agent semantic client.

pub mod cache_artifact;
pub mod cache_manifest;
pub mod config;
pub mod project_context;
pub mod provider_scope;
pub mod receipt;
pub mod request;
pub mod runtime_provider;
pub use agent_semantic_runtime::state_core;
pub mod types;

pub use agent_semantic_tree_sitter::BuiltinCatalogId;
pub use agent_semantic_tree_sitter::BuiltinCatalogLanguageId;
pub use agent_semantic_tree_sitter::LoadedGrammarProfile;
pub use agent_semantic_tree_sitter::LoadedSyntaxCatalog;
pub use agent_semantic_tree_sitter::SyntaxCatalogDescriptor;
pub use agent_semantic_tree_sitter::SyntaxQueryAbiError;
pub use agent_semantic_tree_sitter::SyntaxQueryAbiPattern;
pub use agent_semantic_tree_sitter::SyntaxQueryAbiPlan;
pub use agent_semantic_tree_sitter::builtin_catalog_source;
pub use agent_semantic_tree_sitter::compile_query_abi_source;
pub use agent_semantic_tree_sitter::extract_capture_names;
pub use agent_semantic_tree_sitter::fingerprint_catalog;
pub use agent_semantic_tree_sitter::fingerprint_grammar_profile;
pub use agent_semantic_tree_sitter::load_grammar_profile;
pub use agent_semantic_tree_sitter::load_syntax_catalog;
pub use agent_semantic_tree_sitter::normalize_capture_names;
pub use cache_artifact::replay_artifact_path;
pub use cache_artifact::replay_artifacts_root;
pub use cache_artifact::structured_evidence_artifact_path;
pub use cache_manifest::AGENT_SEMANTIC_CLIENT_CACHE_MANIFEST_FILE;
pub use cache_manifest::AGENT_SEMANTIC_CLIENT_CACHE_MANIFEST_PROTOCOL_ID;
pub use cache_manifest::AGENT_SEMANTIC_CLIENT_CACHE_MANIFEST_PROTOCOL_VERSION;
pub use cache_manifest::AGENT_SEMANTIC_CLIENT_CACHE_MANIFEST_SCHEMA_ID;
pub use cache_manifest::AGENT_SEMANTIC_CLIENT_CACHE_MANIFEST_SCHEMA_VERSION;
pub use cache_manifest::CacheManifestReport;
pub use cache_manifest::CacheManifestStatus;
pub use cache_manifest::ClientCacheFileHash;
pub use cache_manifest::ClientCacheGeneration;
pub use cache_manifest::ClientCacheManifest;
pub use cache_manifest::project_client_cache_dir;
pub use cache_manifest::project_client_cache_dir_read_only;
pub use cache_manifest::project_client_cache_manifest_path;
pub use config::BackendMode;
pub use config::ClientConfig;
pub use config::PrivacyMode;
pub use project_context::ProjectContext;
pub use project_context::StateLayout;
pub use provider_scope::normalize_project_path;
pub use provider_scope::project_child_path;
pub use provider_scope::provider_supports_source_file;
pub use provider_scope::relative_project_path;
pub use provider_scope::scoped_child_path;
pub use receipt::AGENT_SEMANTIC_CLIENT_RECEIPT_PROTOCOL_ID;
pub use receipt::AGENT_SEMANTIC_CLIENT_RECEIPT_SCHEMA_ID;
pub use receipt::ClientDbEngineFeaturesReceipt;
pub use receipt::ClientDbEngineReceipt;
pub use receipt::ClientDbRuntimePragmasReceipt;
pub use receipt::ClientReceipt;
pub use receipt::ExecutionRoute;
pub use receipt::NativeProvenance;
pub use receipt::ProviderCommandReceipt;
pub use request::ASP_SYNTAX_QUERY_CAPTURES_ARG;
pub use request::ASP_SYNTAX_QUERY_FIELDS_ARG;
pub use request::ASP_SYNTAX_QUERY_NODE_TYPES_ARG;
pub use request::ASP_SYNTAX_QUERY_PREDICATES_JSON_ARG;
pub use request::ClientMethod;
pub use request::ClientRequest;
pub use request::SYNTAX_QUERY_AST_ABI_FINGERPRINT_VERSION;
pub use request::append_syntax_query_plan_args;
pub use request::syntax_query_ast_abi_fingerprint;
pub use runtime_provider::ProviderDocumentInventoryCapability;
pub use runtime_provider::ProviderProjectInventoryCapability;
pub use runtime_provider::ProviderSourceInventoryCapabilities;
pub use runtime_provider::RuntimeProvider;
pub use runtime_provider::RuntimeProviderOperation;
pub use runtime_provider::RuntimeProviderProjection;
pub use runtime_provider::RuntimeProviderProjectionEvidence;
pub use types::ByteCount;
pub use types::CacheArtifactId;
pub use types::CacheExportMethod;
pub use types::CacheGenerationId;
pub use types::CacheStatus;
pub use types::ClientCachePath;
pub use types::ClientDbBackend;
pub use types::ClientDbEngineDurability;
pub use types::ClientDbFileName;
pub use types::ClientDbJournalMode;
pub use types::ClientDbStatus;
pub use types::ClientRepoId;
pub use types::ClientScopeId;
pub use types::ClientStateLayoutVersion;
pub use types::ClientWorkspaceId;
pub use types::CompactArtifactId;
pub use types::ElapsedMillis;
pub use types::LanguageId;
pub use types::ProviderId;
pub use types::SemanticProtocolId;
pub use types::SemanticProtocolVersion;
pub use types::SemanticSchemaId;
pub use types::SemanticSchemaVersion;
pub use types::SyntaxQueryAstAbiFingerprint;
pub use types::SyntaxQueryGrammarId;
pub use types::SyntaxQueryGrammarProfileVersion;
pub use types::SyntaxQuerySelector;
#[cfg(test)]
#[path = "../tests/unit/cache_artifact.rs"]
mod cache_artifact_tests;
#[cfg(test)]
#[path = "../tests/unit/cache_manifest.rs"]
mod cache_manifest_tests;
#[cfg(test)]
#[path = "../tests/unit/project_context.rs"]
mod project_context_tests;
#[cfg(test)]
#[path = "../tests/unit/provider_scope.rs"]
mod provider_scope_tests;
#[cfg(test)]
#[path = "../tests/unit/request.rs"]
mod request_tests;
#[cfg(test)]
#[path = "../tests/unit/state_core.rs"]
mod state_core_tests;

#[cfg(test)]
#[path = "../tests/unit/test_support.rs"]
mod test_support;
