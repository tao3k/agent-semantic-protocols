//! Atomic storage for the pending Runtime artifact activation receipt.

use std::path::Path;
use std::path::PathBuf;

pub(crate) fn publish_pending_runtime_artifact_activation(
    path: &Path,
    bytes: &[u8],
    publication_nonce: &str,
) -> Result<(), String> {
    let staged = stage_pending_runtime_artifact_activation(path, bytes, publication_nonce)?;
    commit_staged_pending_runtime_artifact_activation(&staged, path)
}

pub(crate) fn stage_pending_runtime_artifact_activation(
    path: &Path,
    bytes: &[u8],
    publication_nonce: &str,
) -> Result<PathBuf, String> {
    let parent = path
        .parent()
        .ok_or_else(|| "Runtime artifact activation path has no parent".to_owned())?;
    std::fs::create_dir_all(parent)
        .map_err(|error| format!("create Runtime artifact activation directory: {error}"))?;
    let staged = parent.join(format!(".pending-{publication_nonce}.json.tmp"));
    std::fs::write(&staged, bytes)
        .map_err(|error| format!("stage Runtime artifact activation event: {error}"))?;
    Ok(staged)
}

pub(crate) fn commit_staged_pending_runtime_artifact_activation(
    staged: &Path,
    path: &Path,
) -> Result<(), String> {
    std::fs::rename(staged, path)
        .map_err(|error| format!("publish Runtime artifact activation event: {error}"))
}
