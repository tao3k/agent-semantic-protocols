use super::{install_protocol_binary_targets, protocol_binary_artifact_digest};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

#[test]
fn installed_binary_is_blake3_addressed_and_public_target_is_constant_time() {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    let root = std::env::temp_dir().join(format!(
        "asp-binary-blake3-artifact-{}-{nonce}",
        std::process::id()
    ));
    std::fs::create_dir_all(root.join("source")).expect("create source dir");
    std::fs::create_dir_all(root.join("bin")).expect("create bin dir");
    std::fs::create_dir_all(root.join("bin-secondary")).expect("create secondary bin dir");
    let source = root.join("source/asp");
    let target = root.join("bin/asp");
    let secondary_target = root.join("bin-secondary/asp");
    std::fs::write(&source, b"asp artifact one").expect("write source");

    install_protocol_binary_targets(&source, &[target.clone(), secondary_target.clone()])
        .expect("install protocol binary");
    let source_digest = protocol_binary_artifact_digest(&source).expect("source identity");
    let target_digest = protocol_binary_artifact_digest(&target).expect("target identity");
    assert_eq!(source_digest, target_digest);
    let first_artifact = std::fs::canonicalize(&target).expect("first artifact");
    assert_eq!(
        first_artifact,
        std::fs::canonicalize(&secondary_target).expect("secondary artifact")
    );
    assert!(
        first_artifact
            .to_string_lossy()
            .contains("/.asp-artifacts/blake3-256/"),
        "{}",
        first_artifact.display()
    );

    let mut samples = Vec::with_capacity(200);
    for _ in 0..200 {
        let started = Instant::now();
        assert_eq!(
            protocol_binary_artifact_digest(&target),
            Some(target_digest.clone())
        );
        samples.push(started.elapsed());
    }
    samples.sort_unstable();
    let p95 = samples[samples.len() * 95 / 100];
    println!(
        "[protocol-binary-identity-perf] samples={} p95Micros={} budgetMicros=1000",
        samples.len(),
        p95.as_micros()
    );
    assert!(
        p95 < std::time::Duration::from_millis(1),
        "digest-addressed identity p95 exceeded 1ms: {p95:?}"
    );

    std::fs::write(&source, b"asp artifact two with different bytes")
        .expect("replace source binary");
    let changed_digest = protocol_binary_artifact_digest(&source).expect("changed source digest");
    assert_ne!(changed_digest, target_digest);
    assert_eq!(
        protocol_binary_artifact_digest(&target),
        Some(target_digest)
    );
    install_protocol_binary_targets(&source, &[target.clone(), secondary_target.clone()])
        .expect("replace public target");
    let second_artifact = std::fs::canonicalize(&target).expect("second artifact");
    assert_eq!(
        second_artifact,
        std::fs::canonicalize(&secondary_target).expect("secondary replacement")
    );
    assert_ne!(first_artifact, second_artifact);
    assert!(
        first_artifact.is_file(),
        "first artifact must remain immutable"
    );
    assert_eq!(
        protocol_binary_artifact_digest(&target),
        Some(changed_digest)
    );
    std::fs::remove_dir_all(root).expect("cleanup temp root");
}
