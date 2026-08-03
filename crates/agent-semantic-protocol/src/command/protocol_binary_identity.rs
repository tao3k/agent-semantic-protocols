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
    agent_semantic_content_identity::blake3_digest_from_canonical_artifact_path(canonical)
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
    if let Some(digest) = protocol_binary_digest_from_canonical_artifact_path(&canonical) {
        return Ok(digest);
    }
    let identity_path = canonical.clone();
    tokio::task::spawn_blocking(move || {
        agent_semantic_content_identity::file_content_digest_v1(&identity_path).map_err(|error| {
            format!(
                "failed to derive ASP runtime artifact identity from {}: {error}",
                identity_path.display()
            )
        })
    })
    .await
    .map_err(|error| format!("join ASP runtime artifact identity task: {error}"))?
}

fn valid_blake3_digest(digest: &str) -> bool {
    digest.len() == 64
        && digest
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}
