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
mod tests {
    use super::blake3_digest_from_canonical_artifact_path;
    use std::path::Path;

    #[test]
    fn extracts_only_a_canonical_digest_addressed_layout() {
        let digest = "a".repeat(64);
        let path = format!("/state/runtime/artifacts/blake3-256/{digest}/asp");
        assert_eq!(
            blake3_digest_from_canonical_artifact_path(Path::new(&path)).as_deref(),
            Some(digest.as_str())
        );
        assert_eq!(
            blake3_digest_from_canonical_artifact_path(Path::new(
                "/state/runtime/artifacts/latest/asp"
            )),
            None
        );
    }
}
