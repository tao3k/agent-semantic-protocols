//! Structural index artifact import for `asp cache import`.

use std::{fs, path::Path};

use agent_semantic_client_core::{ClientCacheGeneration, ClientCacheManifest};
use agent_semantic_client_db::ClientDbEngine;

use crate::cache_replay::replay_artifact_path;

const SEMANTIC_STRUCTURAL_INDEX_SCHEMA_ID: &str =
    "agent.semantic-protocols.semantic-structural-index";

pub(super) fn import_structural_index_artifacts(
    cache_root: &Path,
    manifest: &ClientCacheManifest,
) -> Result<u64, String> {
    manifest
        .generations
        .iter()
        .try_fold(0, |count, generation| {
            import_structural_index_generation(cache_root, generation)
                .map(|imported| if imported { count + 1 } else { count })
        })
}

fn import_structural_index_generation(
    cache_root: &Path,
    generation: &ClientCacheGeneration,
) -> Result<bool, String> {
    let Some(artifact_path) = structural_index_artifact_path(cache_root, generation)? else {
        return Ok(false);
    };
    let packet_bytes = fs::read(&artifact_path).map_err(|error| {
        format!(
            "failed to read structural index artifact at {}: {error}",
            artifact_path.display()
        )
    })?;
    ClientDbEngine::import_semantic_structural_index_refresh_packet_from_client_dir(
        cache_root,
        generation,
        &packet_bytes,
    )
    .map_err(|error| {
        format!(
            "failed to import structural index artifact for generation {}: {error}",
            generation.generation_id
        )
    })?;
    Ok(true)
}

fn structural_index_artifact_path(
    cache_root: &Path,
    generation: &ClientCacheGeneration,
) -> Result<Option<std::path::PathBuf>, String> {
    if !generation
        .schema_ids
        .iter()
        .any(|schema_id| schema_id.as_str() == SEMANTIC_STRUCTURAL_INDEX_SCHEMA_ID)
    {
        return Ok(None);
    }
    let artifact_ids = generation.artifact_ids.as_ref().ok_or_else(|| {
        format!(
            "structural index generation {} has no artifact ids",
            generation.generation_id
        )
    })?;
    let artifact_id = artifact_ids
        .iter()
        .find(|artifact_id| {
            let artifact_id = artifact_id.as_str();
            artifact_id.starts_with("structural-index/") && artifact_id.ends_with(".json")
        })
        .ok_or_else(|| {
            format!(
                "structural index generation {} has no structural-index artifact",
                generation.generation_id
            )
        })?;
    replay_artifact_path(cache_root, artifact_id, "structural-index/", ".json")
        .map(Some)
        .ok_or_else(|| format!("invalid structural index artifact id `{}`", artifact_id))
}
