// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-only

//! Content identity recovered from canonical digest-addressed artifact paths.

use std::path::Path;

/// Return the BLAKE3 digest encoded by
/// `<artifact-root>/blake3-256/<digest>/<artifact>`.
#[must_use]
pub fn blake3_digest_from_canonical_artifact_path(path: &Path) -> Option<String> {
    let parent = path.parent()?;
    let digest = parent.file_name()?.to_str()?;
    let algorithm = parent.parent()?.file_name()?.to_str()?;
    if algorithm == "blake3-256" && valid_blake3_digest(digest) {
        Some(digest.to_string())
    } else {
        None
    }
}

fn valid_blake3_digest(digest: &str) -> bool {
    digest.len() == 64
        && digest
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

#[cfg(test)]
#[path = "../tests/unit/content_addressed_path.rs"]
mod tests;
