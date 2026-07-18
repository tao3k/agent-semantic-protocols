use super::{install_protocol_binary_targets, protocol_binary_artifact_digest};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

#[test]
fn installed_binary_identity_is_constant_time_and_invalidates_after_source_change() {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    let root = std::env::temp_dir().join(format!(
        "asp-binary-artifact-identity-{}-{nonce}",
        std::process::id()
    ));
    std::fs::create_dir_all(root.join("source")).expect("create source dir");
    std::fs::create_dir_all(root.join("bin")).expect("create bin dir");
    let source = root.join("source/asp");
    let target = root.join("bin/asp");
    std::fs::write(&source, b"asp artifact one").expect("write source");

    install_protocol_binary_targets(&source, std::slice::from_ref(&target))
        .expect("install protocol binary");
    let source_digest = protocol_binary_artifact_digest(&source).expect("source identity");
    let target_digest = protocol_binary_artifact_digest(&target).expect("target identity");
    assert_eq!(source_digest, target_digest);

    let started = Instant::now();
    for _ in 0..64 {
        assert_eq!(
            protocol_binary_artifact_digest(&source),
            protocol_binary_artifact_digest(&target)
        );
    }
    assert!(
        started.elapsed().as_millis() < 250,
        "64 artifact identity comparisons exceeded 250ms: {:?}",
        started.elapsed()
    );

    std::fs::write(&source, b"asp artifact two with different bytes")
        .expect("replace source binary");
    assert_eq!(protocol_binary_artifact_digest(&source), None);
    assert_eq!(
        protocol_binary_artifact_digest(&target),
        Some(target_digest)
    );
    std::fs::remove_dir_all(root).expect("cleanup temp root");
}
