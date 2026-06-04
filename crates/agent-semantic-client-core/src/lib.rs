//! Core contracts for the local-first agent semantic client.

pub mod activation;
pub mod cache_manifest;
pub mod config;
pub mod receipt;
pub mod request;
pub mod types;

pub use activation::{ProviderRegistrySnapshot, ResolvedProvider};
pub use cache_manifest::{
    AGENT_SEMANTIC_CLIENT_CACHE_MANIFEST_FILE, AGENT_SEMANTIC_CLIENT_CACHE_MANIFEST_PROTOCOL_ID,
    AGENT_SEMANTIC_CLIENT_CACHE_MANIFEST_PROTOCOL_VERSION,
    AGENT_SEMANTIC_CLIENT_CACHE_MANIFEST_SCHEMA_ID,
    AGENT_SEMANTIC_CLIENT_CACHE_MANIFEST_SCHEMA_VERSION, CacheManifestReport, CacheManifestStatus,
    ClientCacheFileHash, ClientCacheGeneration, ClientCacheManifest, project_client_cache_dir,
    project_client_cache_manifest_path,
};
pub use config::{BackendMode, ClientConfig, PrivacyMode};
pub use receipt::{
    AGENT_SEMANTIC_CLIENT_RECEIPT_PROTOCOL_ID, AGENT_SEMANTIC_CLIENT_RECEIPT_SCHEMA_ID,
    ClientReceipt, ExecutionRoute, NativeProvenance, ProviderCommandReceipt,
};
pub use request::{ClientMethod, ClientRequest};
pub use types::{
    ByteCount, CacheArtifactId, CacheExportMethod, CacheGenerationId, CacheStatus, ClientCachePath,
    ClientDbStatus, CompactArtifactId, ElapsedMillis, LanguageId, ProviderId, SemanticProtocolId,
    SemanticProtocolVersion, SemanticSchemaId, SemanticSchemaVersion,
};
#[cfg(test)]
#[path = "../tests/unit/cache_manifest.rs"]
mod cache_manifest_tests;
