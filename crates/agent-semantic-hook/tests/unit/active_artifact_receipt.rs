use agent_semantic_content_identity::active_artifact_merkle_v1::ActiveArtifactKindV1;
use agent_semantic_content_identity::exact_selector_merkle::blake3_content_digest_v1;
use agent_semantic_hook::{
    ActiveAspArtifactInput, materialize_active_asp_artifact_receipt,
    verify_active_asp_artifact_receipt,
};
use std::fs;
use std::path::PathBuf;
use std::process;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

fn fixture() -> (PathBuf, PathBuf, PathBuf, String) {
    let root = std::env::temp_dir().join(format!(
        "asp-active-artifact-{}-{}",
        process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos()
    ));
    let binary_bytes = b"asp-binary";
    let digest = blake3_content_digest_v1(binary_bytes).as_str().to_string();
    let binary = root
        .join("bin/.asp-artifacts/blake3-256")
        .join(&digest)
        .join("asp");
    let activation = root.join("state/activation.json");
    fs::create_dir_all(binary.parent().expect("binary parent")).expect("binary parent");
    fs::create_dir_all(activation.parent().expect("activation parent")).expect("activation parent");
    fs::write(&binary, binary_bytes).expect("binary");
    fs::write(&activation, br#"{"schemaVersion":"1"}"#).expect("activation");
    (root, binary, activation, digest)
}

#[test]
fn receipt_binds_materialized_targets_and_rejects_drift() {
    let (root, binary, activation, digest) = fixture();
    let provider = root.join("providers/rs-harness");
    fs::create_dir_all(provider.parent().expect("provider parent")).expect("provider parent");
    fs::write(&provider, b"provider").expect("provider");
    let additional = [ActiveAspArtifactInput {
        logical_path: "providers/rust/rs-harness".to_string(),
        artifact_kind: ActiveArtifactKindV1::ProviderBinary,
        materialized_path: provider.clone(),
    }];
    let materialized =
        materialize_active_asp_artifact_receipt(&binary, &digest, &activation, &additional)
            .expect("materialize");
    assert!(materialized.receipt_path.is_file());
    let verified =
        verify_active_asp_artifact_receipt(&activation, &[&binary]).expect("verified receipt");
    assert_eq!(verified, materialized.receipt);
    assert_eq!(verified.leaves.len(), 3);

    fs::write(&provider, b"provider-drift").expect("drift provider");
    let error = verify_active_asp_artifact_receipt(&activation, &[&binary])
        .expect_err("provider drift must fail");
    assert!(error.contains("provider-binary size mismatch"), "{error}");
    fs::write(&provider, b"provider").expect("restore provider");

    fs::write(&activation, br#"{"schemaVersion":"1","drift":true}"#).expect("drift activation");
    let error =
        verify_active_asp_artifact_receipt(&activation, &[&binary]).expect_err("drift must fail");
    assert!(error.contains("activation size mismatch"), "{error}");
    fs::remove_dir_all(root).expect("remove fixture");
}

#[test]
fn warm_receipt_metadata_verification_p95_is_under_ten_milliseconds() {
    let (root, binary, activation, digest) = fixture();
    let mut providers = Vec::new();
    let mut additional = Vec::new();
    for index in 0..7 {
        let provider = root.join(format!("providers/provider-{index}"));
        fs::create_dir_all(provider.parent().expect("provider parent")).expect("provider parent");
        fs::write(&provider, format!("provider-{index}")).expect("provider");
        additional.push(ActiveAspArtifactInput {
            logical_path: format!("providers/language-{index}/provider-{index}"),
            artifact_kind: ActiveArtifactKindV1::ProviderBinary,
            materialized_path: provider.clone(),
        });
        providers.push(provider);
    }
    materialize_active_asp_artifact_receipt(&binary, &digest, &activation, &additional)
        .expect("materialize");
    verify_active_asp_artifact_receipt(&activation, &[&binary]).expect("warmup");

    let mut samples = Vec::with_capacity(200);
    for _ in 0..200 {
        let started = Instant::now();
        verify_active_asp_artifact_receipt(&activation, &[&binary]).expect("verify");
        samples.push(started.elapsed());
    }
    samples.sort_unstable();
    let p95 = samples[samples.len() * 95 / 100];
    println!(
        "[active-artifact-perf] samples={} p95Micros={} budgetMicros=10000",
        samples.len(),
        p95.as_micros()
    );
    assert!(p95 < Duration::from_millis(10), "p95={p95:?}");
    fs::remove_dir_all(root).expect("remove fixture");
}
