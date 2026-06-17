#![deny(dead_code)]

//! Core contracts for the local-first agent semantic client.

pub mod activation;
pub mod cache_manifest;
pub mod config;
pub mod project_context;
pub mod receipt;
pub mod request;
pub mod types;

pub use activation::{
    ASP_PROVIDER_ACTIVATION_PATH_ENV, ProviderRegistrySnapshot, ResolvedProvider,
    RuntimeProfileStatus,
};
pub use agent_semantic_config::ProjectEnvStatus;
pub use agent_semantic_hook::ProviderExecution;
pub use cache_manifest::{
    AGENT_SEMANTIC_CLIENT_CACHE_MANIFEST_FILE, AGENT_SEMANTIC_CLIENT_CACHE_MANIFEST_PROTOCOL_ID,
    AGENT_SEMANTIC_CLIENT_CACHE_MANIFEST_PROTOCOL_VERSION,
    AGENT_SEMANTIC_CLIENT_CACHE_MANIFEST_SCHEMA_ID,
    AGENT_SEMANTIC_CLIENT_CACHE_MANIFEST_SCHEMA_VERSION, CacheManifestReport, CacheManifestStatus,
    ClientCacheFileHash, ClientCacheGeneration, ClientCacheManifest, project_client_cache_dir,
    project_client_cache_manifest_path,
};
pub use config::{BackendMode, ClientConfig, PrivacyMode};
pub use project_context::{ProjectContext, StateLayout};
pub use receipt::{
    AGENT_SEMANTIC_CLIENT_RECEIPT_PROTOCOL_ID, AGENT_SEMANTIC_CLIENT_RECEIPT_SCHEMA_ID,
    ClientReceipt, ExecutionRoute, NativeProvenance, ProviderCommandReceipt,
};
pub use types::{
    ByteCount, CacheArtifactId, CacheExportMethod, CacheGenerationId, CacheStatus, ClientCachePath,
    ClientDbJournalMode, ClientDbStatus, CompactArtifactId, ElapsedMillis, LanguageId, ProviderId,
    SemanticProtocolId, SemanticProtocolVersion, SemanticSchemaId, SemanticSchemaVersion,
    SyntaxQueryAstAbiFingerprint, SyntaxQueryGrammarId, SyntaxQueryGrammarProfileVersion,
    SyntaxQuerySelector,
};
pub use {
    agent_semantic_tree_sitter::{
        BuiltinCatalogId, BuiltinCatalogLanguageId, CompiledSyntaxQuery, LoadedGrammarProfile,
        LoadedSyntaxCatalog, SyntaxCatalogDescriptor, SyntaxQueryAbiError, SyntaxQueryAbiPattern,
        SyntaxQueryAbiPlan, SyntaxQueryCompileError, builtin_catalog_source, compile_catalog_query,
        compile_query_abi_source, compile_query_source, extract_capture_names, fingerprint_catalog,
        fingerprint_grammar_profile, load_grammar_profile, load_syntax_catalog,
        normalize_capture_names,
    },
    request::{
        ASP_SYNTAX_QUERY_CAPTURES_ARG, ASP_SYNTAX_QUERY_FIELDS_ARG,
        ASP_SYNTAX_QUERY_NODE_TYPES_ARG, ASP_SYNTAX_QUERY_PREDICATES_JSON_ARG, ClientMethod,
        ClientRequest, SYNTAX_QUERY_AST_ABI_FINGERPRINT_VERSION, append_syntax_query_plan_args,
        syntax_query_ast_abi_fingerprint,
    },
};
#[cfg(test)]
#[path = "../tests/unit/activation.rs"]
mod activation_tests;
#[cfg(test)]
#[path = "../tests/unit/cache_manifest.rs"]
mod cache_manifest_tests;
#[cfg(test)]
#[path = "../tests/unit/project_context.rs"]
mod project_context_tests;
#[cfg(test)]
#[path = "../tests/unit/request.rs"]
mod request_tests;
