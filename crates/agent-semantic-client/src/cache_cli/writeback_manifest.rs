//! Cache manifest helpers for write-back flows.

use std::fs;
use std::path::Path;

use agent_semantic_client_core::{
    AGENT_SEMANTIC_CLIENT_CACHE_MANIFEST_PROTOCOL_ID,
    AGENT_SEMANTIC_CLIENT_CACHE_MANIFEST_PROTOCOL_VERSION,
    AGENT_SEMANTIC_CLIENT_CACHE_MANIFEST_SCHEMA_ID,
    AGENT_SEMANTIC_CLIENT_CACHE_MANIFEST_SCHEMA_VERSION, CacheManifestStatus,
    ClientCacheGeneration, ClientCacheManifest, ClientCachePath,
};

pub(super) fn load_existing_or_empty_manifest(
    cache_root: &Path,
    manifest_path: &Path,
    status: &CacheManifestStatus,
) -> ClientCacheManifest {
    match status {
        CacheManifestStatus::Missing
        | CacheManifestStatus::Invalid
        | CacheManifestStatus::Unavailable => empty_cache_manifest(cache_root),
        CacheManifestStatus::Present => ClientCacheManifest::load_from_path(manifest_path)
            .unwrap_or_else(|_| empty_cache_manifest(cache_root)),
    }
}

pub(super) fn upsert_generation(
    manifest: &mut ClientCacheManifest,
    generation: ClientCacheGeneration,
) {
    manifest
        .generations
        .retain(|existing| existing.generation_id != generation.generation_id);
    manifest.generations.push(generation);
}

pub(super) fn write_cache_manifest(
    manifest_path: &Path,
    manifest: &ClientCacheManifest,
) -> Result<(), String> {
    if let Some(parent) = manifest_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            format!(
                "failed to create agent semantic client cache manifest directory {}: {error}",
                parent.display()
            )
        })?;
    }
    let text = serde_json::to_string_pretty(manifest)
        .map_err(|error| format!("failed to serialize cache manifest: {error}"))?;
    fs::write(manifest_path, text).map_err(|error| {
        format!(
            "failed to write agent semantic client cache manifest at {}: {error}",
            manifest_path.display()
        )
    })
}

fn empty_cache_manifest(cache_root: &Path) -> ClientCacheManifest {
    ClientCacheManifest {
        schema_id: AGENT_SEMANTIC_CLIENT_CACHE_MANIFEST_SCHEMA_ID.into(),
        schema_version: AGENT_SEMANTIC_CLIENT_CACHE_MANIFEST_SCHEMA_VERSION.into(),
        protocol_id: AGENT_SEMANTIC_CLIENT_CACHE_MANIFEST_PROTOCOL_ID.into(),
        protocol_version: AGENT_SEMANTIC_CLIENT_CACHE_MANIFEST_PROTOCOL_VERSION.into(),
        cache_root: ClientCachePath::from_path(cache_root),
        generations: Vec::new(),
    }
}
