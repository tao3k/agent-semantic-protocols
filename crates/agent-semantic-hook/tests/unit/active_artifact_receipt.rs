use agent_semantic_content_identity::exact_selector_merkle::blake3_content_digest_v1;
use agent_semantic_hook::materialize_active_asp_artifact_receipt;
use std::fs;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;

#[test]
fn unchanged_asp_artifacts_are_zero_byte_read_and_zero_write() {
    let root = std::env::temp_dir().join(format!(
        "asp-active-artifact-warm-path-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time")
            .as_nanos()
    ));
    fs::create_dir_all(&root).expect("create test root");
    let binary_path = root.join("asp");
    let activation_path = root.join("activation.json");
    let binary_bytes = b"asp-test-binary";
    fs::write(&binary_path, binary_bytes).expect("write binary");
    fs::write(&activation_path, b"{}").expect("write activation");
    let binary_digest =
        agent_semantic_artifacts::blake3_content_digest::Blake3ContentDigest::from_content_digest(
            blake3_content_digest_v1(binary_bytes),
        );

    let cold = materialize_active_asp_artifact_receipt(
        &binary_path,
        binary_digest.as_str(),
        &activation_path,
    )
    .expect("cold materialization");
    assert_eq!(cold.artifact_byte_reads, 0);
    assert_eq!(cold.artifact_bytes_read, 0);
    assert_eq!(cold.receipt_writes, 1);

    let started = std::time::Instant::now();
    let warm = materialize_active_asp_artifact_receipt(
        &binary_path,
        binary_digest.as_str(),
        &activation_path,
    )
    .expect("warm materialization");
    let elapsed = started.elapsed();
    assert_eq!(warm.artifact_byte_reads, 0);
    assert_eq!(warm.artifact_bytes_read, 0);
    assert_eq!(warm.receipt_writes, 0);
    assert_eq!(warm.receipt, cold.receipt);
    assert!(
        elapsed < std::time::Duration::from_millis(100),
        "warm materialization must remain millisecond-scale, elapsed={elapsed:?}"
    );

    fs::remove_dir_all(root).expect("remove test root");
}

#[test]
fn cli_active_artifact_composition_roundtrips_precise_typed_digest() {
    const DIGEST: &str =
        "blake3-256:9313893f2985088dc3e5b14fdfd4877b0ed37c8dc8d5ef60bd6531ab5678abe1";
    let root = std::env::temp_dir().join(format!(
        "asp-active-artifact-typed-digest-{}",
        std::process::id()
    ));
    fs::create_dir_all(&root).expect("create typed digest fixture");
    let binary_path = root.join("asp");
    let activation_path = root.join("activation.json");
    fs::write(&binary_path, b"typed-digest-fixture").expect("write fixture binary");
    fs::write(&activation_path, b"{}").expect("write activation");

    let cold = materialize_active_asp_artifact_receipt(&binary_path, DIGEST, &activation_path)
        .expect("accept canonical typed digest");
    let warm = materialize_active_asp_artifact_receipt(&binary_path, DIGEST, &activation_path)
        .expect("roundtrip canonical typed digest");
    assert_eq!(warm.receipt, cold.receipt);

    for invalid in [
        &DIGEST["blake3-256:".len()..],
        "blake3-256:9313893f",
        "blake3-256:9313893F2985088DC3E5B14FDFD4877B0ED37C8DC8D5EF60BD6531AB5678ABE1",
        "blake3-256:blake3-256:9313893f2985088dc3e5b14fdfd4877b0ed37c8dc8d5ef60bd6531ab5678abe1",
        "blake3-256:9313893g2985088dc3e5b14fdfd4877b0ed37c8dc8d5ef60bd6531ab5678abe1",
    ] {
        let error =
            materialize_active_asp_artifact_receipt(&binary_path, invalid, &activation_path)
                .expect_err("reject malformed digest");
        assert!(error.contains("invalid BLAKE3 ASP binary digest"));
    }
    fs::remove_dir_all(root).expect("remove typed digest fixture");
}
