// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

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
    Ok(store == "generations"
        && valid_blake3_digest(digest)
        && !binary.is_empty()
        && components.next().is_none())
}

pub(crate) fn protocol_binary_digest_from_canonical_artifact_path(
    canonical: &Path,
) -> Option<String> {
    agent_semantic_content_identity::file_content_digest_v1(canonical).ok()
}

fn valid_blake3_digest(digest: &str) -> bool {
    digest.len() == 64
        && digest
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}
