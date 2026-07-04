//! Cache manifest model and path resolution for `agent-semantic-client`.

use std::fs;
use std::path::{Path, PathBuf};

use crate::types::{
    CacheArtifactId, CacheGenerationId, CacheStatus, ClientCachePath, LanguageId, ProviderId,
    SemanticProtocolId, SemanticProtocolVersion, SemanticSchemaId, SemanticSchemaVersion,
};
use serde::{Deserialize, Serialize};

/// Schema id for `agent-semantic-client-cache-manifest.v1`.
pub const AGENT_SEMANTIC_CLIENT_CACHE_MANIFEST_SCHEMA_ID: &str =
    "agent.semantic-protocols.client-cache-manifest";
/// Protocol id used by the client cache manifest envelope.
pub const AGENT_SEMANTIC_CLIENT_CACHE_MANIFEST_PROTOCOL_ID: &str =
    "agent.semantic-protocols.client";
/// Schema version for `agent-semantic-client-cache-manifest.v1`.
pub const AGENT_SEMANTIC_CLIENT_CACHE_MANIFEST_SCHEMA_VERSION: &str = "1";
/// Protocol version for client cache manifests.
pub const AGENT_SEMANTIC_CLIENT_CACHE_MANIFEST_PROTOCOL_VERSION: &str = "1";
/// File name for the JSON cache manifest beside the Turso client DB.
pub const AGENT_SEMANTIC_CLIENT_CACHE_MANIFEST_FILE: &str = "cache-manifest.json";

/// Filesystem and schema status for a cache manifest inspection.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum CacheManifestStatus {
    Unavailable,
    Missing,
    Present,
    Invalid,
}

impl CacheManifestStatus {
    #[must_use]
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Unavailable => "unavailable",
            Self::Missing => "missing",
            Self::Present => "present",
            Self::Invalid => "invalid",
        }
    }
}

/// Read-only status report for a cache manifest path.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CacheManifestReport {
    pub cache_root: Option<PathBuf>,
    pub manifest_path: Option<PathBuf>,
    pub status: CacheManifestStatus,
    pub generation_count: u32,
    pub raw_source_stored: bool,
    pub reason: Option<String>,
}

impl CacheManifestReport {
    #[must_use]
    fn unavailable(reason: String) -> Self {
        Self {
            cache_root: None,
            manifest_path: None,
            status: CacheManifestStatus::Unavailable,
            generation_count: 0,
            raw_source_stored: false,
            reason: Some(reason),
        }
    }

    #[must_use]
    fn missing(cache_root: PathBuf, manifest_path: PathBuf) -> Self {
        Self {
            cache_root: Some(cache_root),
            manifest_path: Some(manifest_path),
            status: CacheManifestStatus::Missing,
            generation_count: 0,
            raw_source_stored: false,
            reason: None,
        }
    }

    #[must_use]
    fn present(manifest_path: PathBuf, manifest: &ClientCacheManifest) -> Self {
        Self {
            cache_root: Some(PathBuf::from(manifest.cache_root.as_ref())),
            manifest_path: Some(manifest_path),
            status: CacheManifestStatus::Present,
            generation_count: manifest.generations.len().min(u32::MAX as usize) as u32,
            raw_source_stored: manifest.raw_source_stored(),
            reason: None,
        }
    }

    #[must_use]
    fn invalid(
        cache_root: PathBuf,
        manifest_path: PathBuf,
        generation_count: u32,
        raw_source_stored: bool,
        reason: String,
    ) -> Self {
        Self {
            cache_root: Some(cache_root),
            manifest_path: Some(manifest_path),
            status: CacheManifestStatus::Invalid,
            generation_count,
            raw_source_stored,
            reason: Some(reason),
        }
    }
}

/// JSON cache manifest describing provider-owned cache generations.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClientCacheManifest {
    pub schema_id: SemanticSchemaId,
    pub schema_version: SemanticSchemaVersion,
    pub protocol_id: SemanticProtocolId,
    pub protocol_version: SemanticProtocolVersion,
    pub cache_root: ClientCachePath,
    pub generations: Vec<ClientCacheGeneration>,
}

impl ClientCacheManifest {
    /// Inspect the default manifest location for a project root.
    pub fn inspect_project(project_root: &Path) -> CacheManifestReport {
        let manifest_path = match project_client_cache_manifest_path(project_root) {
            Ok(path) => path,
            Err(error) => return CacheManifestReport::unavailable(error),
        };
        let cache_root = manifest_path
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| PathBuf::from("."));
        if !manifest_path.exists() {
            return CacheManifestReport::missing(cache_root, manifest_path);
        }

        match Self::load_from_path(&manifest_path) {
            Ok(manifest) => CacheManifestReport::present(manifest_path, &manifest),
            Err(error) => {
                let (generation_count, raw_source_stored) =
                    Self::best_effort_manifest_counts(&manifest_path);
                CacheManifestReport::invalid(
                    cache_root,
                    manifest_path,
                    generation_count,
                    raw_source_stored,
                    error,
                )
            }
        }
    }

    /// Load and validate a cache manifest from an explicit path.
    pub fn load_from_path(manifest_path: &Path) -> Result<Self, String> {
        let text = fs::read_to_string(manifest_path).map_err(|error| {
            format!(
                "failed to read agent semantic client cache manifest at {}: {error}",
                manifest_path.display()
            )
        })?;
        let manifest: Self = serde_json::from_str(&text).map_err(|error| {
            format!(
                "failed to parse agent semantic client cache manifest at {}: {error}",
                manifest_path.display()
            )
        })?;
        manifest.validate_contract()?;
        Ok(manifest)
    }

    fn validate_contract(&self) -> Result<(), String> {
        if self.schema_id.as_str() != AGENT_SEMANTIC_CLIENT_CACHE_MANIFEST_SCHEMA_ID {
            return Err(format!("unexpected schemaId `{}`", self.schema_id));
        }
        if self.schema_version.as_str() != AGENT_SEMANTIC_CLIENT_CACHE_MANIFEST_SCHEMA_VERSION {
            return Err(format!(
                "unexpected schemaVersion `{}`",
                self.schema_version
            ));
        }
        if self.protocol_id.as_str() != AGENT_SEMANTIC_CLIENT_CACHE_MANIFEST_PROTOCOL_ID {
            return Err(format!("unexpected protocolId `{}`", self.protocol_id));
        }
        if self.protocol_version.as_str() != AGENT_SEMANTIC_CLIENT_CACHE_MANIFEST_PROTOCOL_VERSION {
            return Err(format!(
                "unexpected protocolVersion `{}`",
                self.protocol_version
            ));
        }
        if self.cache_root.is_empty() {
            return Err("cacheRoot must not be empty".to_string());
        }
        for generation in &self.generations {
            generation.validate_contract()?;
        }
        Ok(())
    }

    fn raw_source_stored(&self) -> bool {
        self.generations
            .iter()
            .any(|generation| generation.raw_source_stored)
    }

    fn best_effort_manifest_counts(manifest_path: &Path) -> (u32, bool) {
        let Ok(text) = fs::read_to_string(manifest_path) else {
            return (0, false);
        };
        let Ok(manifest) = serde_json::from_str::<Self>(&text) else {
            return (0, false);
        };
        (
            manifest.generations.len().min(u32::MAX as usize) as u32,
            manifest.raw_source_stored(),
        )
    }
}

/// One provider-owned cache generation advertised to the client DB.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClientCacheGeneration {
    pub generation_id: CacheGenerationId,
    pub language_id: LanguageId,
    pub provider_id: ProviderId,
    pub provider_version: Option<String>,
    pub export_method: Option<String>,
    pub project_root: String,
    pub package_root: Option<String>,
    pub schema_ids: Vec<SemanticSchemaId>,
    pub cache_status: CacheStatus,
    pub raw_source_stored: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub request_fingerprint: Option<String>,
    pub file_hashes: Option<Vec<ClientCacheFileHash>>,
    pub artifact_ids: Option<Vec<CacheArtifactId>>,
}

impl ClientCacheGeneration {
    fn validate_contract(&self) -> Result<(), String> {
        if self.generation_id.is_empty() {
            return Err("cache generation id cannot be empty".to_string());
        }
        if self.language_id.is_empty() {
            return Err("cache language id cannot be empty".to_string());
        }
        if self.provider_id.is_empty() {
            return Err("cache provider id cannot be empty".to_string());
        }
        if self.project_root.is_empty() {
            return Err("cache projectRoot cannot be empty".to_string());
        }
        if matches!(self.request_fingerprint.as_deref(), Some("")) {
            return Err("cache requestFingerprint cannot be empty".to_string());
        }
        if self.schema_ids.is_empty() {
            return Err("cache schema ids cannot be empty".to_string());
        }
        if self.raw_source_stored {
            return Err("raw source must not be stored".to_string());
        }
        Ok(())
    }
}

/// File hash entry retained for cache invalidation.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClientCacheFileHash {
    pub path: String,
    pub sha256: String,
    pub byte_len: u64,
    pub mtime_ms: u64,
}

/// Return the agent semantic client cache directory for an activated project.
///
/// `agent-semantic-config` owns project identity and state storage layout so
/// client, hook, and provider receipts resolve the same manifest and Turso DB.
pub fn project_client_cache_dir(project_root: impl AsRef<Path>) -> Result<PathBuf, String> {
    let resolved = crate::state_core::ResolvedState::resolve(project_root.as_ref())?;
    resolved.ensure_minimal_layout()?;
    Ok(resolved.paths.client_dir)
}

/// Return the client cache directory without creating or updating state files.
///
/// Hot read paths, such as source-index lookup, use this after a refresh has
/// already created the layout so lookup latency is not dominated by manifest
/// writes.
pub fn project_client_cache_dir_read_only(
    project_root: impl AsRef<Path>,
) -> Result<PathBuf, String> {
    let resolved = crate::state_core::ResolvedState::resolve(project_root.as_ref())?;
    Ok(resolved.paths.client_dir)
}

/// Resolve the JSON cache manifest path for a project.
pub fn project_client_cache_manifest_path(
    project_root: impl AsRef<Path>,
) -> Result<PathBuf, String> {
    Ok(project_client_cache_dir(project_root)?.join(AGENT_SEMANTIC_CLIENT_CACHE_MANIFEST_FILE))
}
