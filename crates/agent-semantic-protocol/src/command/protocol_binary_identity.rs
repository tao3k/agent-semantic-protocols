//! Digest-addressed identity for installed protocol binaries.

use std::fs;
use std::path::Path;

pub(super) fn is_digest_addressed_protocol_binary(
    identity: &Path,
    artifact_root: &Path,
) -> Result<bool, String> {
    let artifact_root = match fs::canonicalize(artifact_root) {
        Ok(artifact_root) => artifact_root,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(error) => {
            return Err(format!(
                "failed to resolve protocol artifact root {}: {error}",
                artifact_root.display()
            ));
        }
    };
    let Ok(relative) = identity.strip_prefix(&artifact_root) else {
        return Ok(false);
    };
    let mut components = relative.components();
    let Some(store) = components
        .next()
        .and_then(|value| value.as_os_str().to_str())
    else {
        return Ok(false);
    };
    let Some(digest) = components
        .next()
        .and_then(|value| value.as_os_str().to_str())
    else {
        return Ok(false);
    };
    let Some(binary) = components.next().map(|value| value.as_os_str()) else {
        return Ok(false);
    };
    Ok(store == "blake3-256"
        && valid_blake3_digest(digest)
        && !binary.is_empty()
        && components.next().is_none())
}

pub(crate) fn protocol_binary_artifact_path_digest(path: &Path) -> Option<String> {
    let canonical = fs::canonicalize(path).ok()?;
    protocol_binary_digest_from_canonical_artifact_path(&canonical)
}

pub(crate) fn protocol_binary_digest_from_canonical_artifact_path(
    canonical: &Path,
) -> Option<String> {
    let parent = canonical.parent()?;
    let digest = parent.file_name()?.to_str()?;
    let algorithm = parent.parent()?.file_name()?.to_str()?;
    if algorithm == "blake3-256" && valid_blake3_digest(digest) {
        Some(digest.to_string())
    } else {
        None
    }
}

pub(crate) async fn canonical_protocol_binary_artifact_digest(
    path: &Path,
) -> Result<String, String> {
    let canonical = tokio::fs::canonicalize(path).await.map_err(|error| {
        format!(
            "failed to resolve canonical ASP runtime artifact {}: {error}",
            path.display()
        )
    })?;
    protocol_binary_digest_from_canonical_artifact_path(&canonical).ok_or_else(|| {
        format!(
            "ASP Runtime Server requires a canonical digest-addressed artifact: {}",
            canonical.display()
        )
    })
}

pub(super) fn protocol_binary_artifact_digest(path: &Path) -> Option<String> {
    if let Some(digest) = protocol_binary_artifact_path_digest(path) {
        return Some(digest);
    }
    let bytes = fs::read(path).ok()?;
    Some(
        agent_semantic_content_identity::exact_selector_merkle::blake3_content_digest_v1(&bytes)
            .as_str()
            .to_string(),
    )
}

fn valid_blake3_digest(digest: &str) -> bool {
    digest.len() == 64
        && digest
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}
