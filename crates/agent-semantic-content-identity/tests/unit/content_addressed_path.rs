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
