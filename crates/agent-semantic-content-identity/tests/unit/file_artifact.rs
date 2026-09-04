use super::file_artifact_metadata_digest_v1;
use super::file_content_digest_v1;
use std::fs;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;

#[test]
fn file_identity_streams_content_and_detects_metadata_drift() {
    let root = std::env::temp_dir().join(format!(
        "asp-file-artifact-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time")
            .as_nanos()
    ));
    fs::create_dir_all(&root).expect("create fixture");
    let artifact = root.join("provider");
    let bytes = vec![0x5a; 16 * 1024 * 1024];
    fs::write(&artifact, &bytes).expect("write artifact");

    let content_digest = file_content_digest_v1(&artifact).expect("content digest");
    assert_eq!(content_digest, blake3::hash(&bytes).to_hex().to_string());
    let metadata_digest = file_artifact_metadata_digest_v1(&artifact).expect("metadata digest");

    fs::write(&artifact, b"drift").expect("drift artifact");
    let drifted_metadata_digest =
        file_artifact_metadata_digest_v1(&artifact).expect("drifted metadata digest");
    assert_ne!(drifted_metadata_digest, metadata_digest);

    fs::remove_dir_all(root).expect("remove fixture");
}
