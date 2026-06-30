//! Cache artifact path contracts.

use std::path::{Component, Path, PathBuf};

use crate::CacheArtifactId;

/// Resolve an artifact id below the workspace artifacts root.
///
/// The caller supplies the State Core live client directory. Only ids with the
/// expected prefix/suffix are accepted, and absolute or parent-relative paths
/// are rejected before joining.
#[must_use]
pub fn replay_artifact_path(
    cache_root: &Path,
    artifact_id: &CacheArtifactId,
    allowed_prefix: &str,
    allowed_suffix: &str,
) -> Option<PathBuf> {
    let artifact_id = artifact_id.as_str();
    if !artifact_id.starts_with(allowed_prefix) || !artifact_id.ends_with(allowed_suffix) {
        return None;
    }
    let relative = Path::new(artifact_id);
    if relative.components().any(|component| {
        matches!(
            component,
            Component::ParentDir | Component::RootDir | Component::Prefix(_)
        )
    }) {
        return None;
    }
    Some(replay_artifacts_root(cache_root)?.join(relative))
}

/// Resolve the workspace artifacts root from a State Core live client dir.
#[must_use]
pub fn replay_artifacts_root(cache_root: &Path) -> Option<PathBuf> {
    let live_dir = cache_root.parent()?;
    if cache_root.file_name().and_then(|name| name.to_str()) == Some("client")
        && live_dir.file_name().and_then(|name| name.to_str()) == Some("live")
    {
        return live_dir
            .parent()
            .map(|workspace_dir| workspace_dir.join("artifacts"));
    }
    None
}

/// Resolve structured evidence artifacts that are safe to replay by schema.
#[must_use]
pub fn structured_evidence_artifact_path(
    cache_root: &Path,
    artifact_id: &CacheArtifactId,
) -> Option<PathBuf> {
    [
        ("relation-plan/", ".json"),
        ("flow-lite/", ".json"),
        ("codeql-evidence/", ".json"),
    ]
    .into_iter()
    .find_map(|(prefix, suffix)| replay_artifact_path(cache_root, artifact_id, prefix, suffix))
}
