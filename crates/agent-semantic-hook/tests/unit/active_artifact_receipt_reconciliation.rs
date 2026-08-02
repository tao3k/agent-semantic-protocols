use std::time::{SystemTime, UNIX_EPOCH};

use super::{
    ActiveAspArtifactInput, ActiveAspArtifactReconciliationV1, atomic_write_compare_exchange,
    materialize_active_asp_artifact_receipt, rebind_active_asp_binary_receipt_if_present,
    reconcile_active_asp_artifact_receipt_if_present, verify_active_asp_artifact_receipt,
};
use agent_semantic_content_identity::active_artifact_merkle_v1::ActiveArtifactKindV1;

fn fixture_root(label: &str) -> std::path::PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    std::env::temp_dir().join(format!(
        "asp-active-artifact-reconciliation-{label}-{}-{nonce}",
        std::process::id()
    ))
}

#[test]
fn concurrent_receipt_publishers_admit_exactly_one_matching_base() {
    let root = fixture_root("publication-cas");
    let receipt = root.join("hooks/state/active-asp-artifact-receipt.v1.json");
    let base = br#"{"root":"base"}"#;
    atomic_write_compare_exchange(&receipt, base, None).expect("publish base receipt");

    let publisher_count = std::thread::available_parallelism()
        .map(std::num::NonZeroUsize::get)
        .unwrap_or(1)
        .max(2);
    let barrier = std::sync::Arc::new(std::sync::Barrier::new(publisher_count));
    let mut publishers = Vec::with_capacity(publisher_count);
    for publisher in 0..publisher_count {
        let barrier = std::sync::Arc::clone(&barrier);
        let receipt = receipt.clone();
        publishers.push(std::thread::spawn(move || {
            let candidate = format!(r#"{{"root":"candidate-{publisher}"}}"#).into_bytes();
            barrier.wait();
            let result = atomic_write_compare_exchange(&receipt, &candidate, Some(base));
            (candidate, result)
        }));
    }

    let outcomes = publishers
        .into_iter()
        .map(|publisher| publisher.join().expect("join receipt publisher"))
        .collect::<Vec<_>>();
    assert_eq!(
        outcomes.iter().filter(|(_, result)| result.is_ok()).count(),
        1,
        "one current-base publisher must win"
    );
    assert!(
        outcomes
            .iter()
            .filter(|(_, result)| result.is_err())
            .all(|(_, result)| result
                .as_ref()
                .expect_err("stale publisher must fail")
                .contains("compare-and-swap conflict"))
    );
    let published = std::fs::read(&receipt).expect("read winning receipt");
    assert!(
        outcomes
            .iter()
            .any(|(candidate, result)| result.is_ok() && candidate == &published),
        "active receipt must equal the one admitted complete candidate"
    );

    std::fs::remove_dir_all(root).expect("remove fixture");
}

#[test]
fn missing_receipt_is_a_typed_bootstrap_state() {
    let root = fixture_root("missing");
    let activation = root.join("hooks/state/activation.json");
    std::fs::create_dir_all(activation.parent().expect("activation parent"))
        .expect("create activation parent");
    std::fs::write(&activation, b"{}").expect("write activation");

    assert_eq!(
        reconcile_active_asp_artifact_receipt_if_present(&activation)
            .expect("inspect optional active artifact receipt"),
        ActiveAspArtifactReconciliationV1::NotMaterialized
    );

    std::fs::remove_dir_all(root).expect("remove fixture");
}

#[test]
fn stale_receipt_without_activation_does_not_block_global_binary_install() {
    let root = fixture_root("stale-receipt");
    let binary = root.join("runtime/bin/asp");
    let activation = root.join("hooks/state/activation.json");
    let receipt = activation
        .parent()
        .expect("activation parent")
        .join("active-asp-artifact-receipt.v1.json");
    std::fs::create_dir_all(binary.parent().expect("binary parent")).expect("create binary parent");
    std::fs::create_dir_all(receipt.parent().expect("receipt parent"))
        .expect("create receipt parent");
    std::fs::write(&binary, b"asp-current").expect("write binary");
    std::fs::write(&receipt, b"{\"stale\":true}").expect("write stale receipt");
    let binary_digest =
        agent_semantic_content_identity::file_content_digest_v1(&binary).expect("binary digest");

    assert_eq!(
        rebind_active_asp_binary_receipt_if_present(&binary, &binary_digest, &activation)
            .expect("missing project activation is not a global install failure"),
        ActiveAspArtifactReconciliationV1::NotMaterialized
    );

    std::fs::remove_dir_all(root).expect("remove fixture");
}

#[test]
fn materialized_receipt_is_reconciled_after_activation_changes() {
    let root = fixture_root("updated");
    let binary = root.join("runtime/bin/asp");
    let activation = root.join("hooks/state/activation.json");
    std::fs::create_dir_all(binary.parent().expect("binary parent")).expect("create binary parent");
    std::fs::create_dir_all(activation.parent().expect("activation parent"))
        .expect("create activation parent");
    std::fs::write(&binary, b"asp-v1").expect("write binary");
    std::fs::write(&activation, b"{\"generation\":1}").expect("write activation");
    let binary_digest =
        agent_semantic_content_identity::file_content_digest_v1(&binary).expect("binary digest");
    materialize_active_asp_artifact_receipt(&binary, &binary_digest, &activation, &[])
        .expect("materialize receipt");

    assert_eq!(
        reconcile_active_asp_artifact_receipt_if_present(&activation)
            .expect("reconcile current receipt"),
        ActiveAspArtifactReconciliationV1::Current
    );

    std::fs::write(&activation, b"{\"generation\":2,\"rankers\":[]}").expect("update activation");
    assert_eq!(
        reconcile_active_asp_artifact_receipt_if_present(&activation)
            .expect("reconcile changed receipt"),
        ActiveAspArtifactReconciliationV1::Updated
    );

    std::fs::write(&binary, b"asp-v2-with-a-new-size").expect("update binary");
    let changed_binary_digest =
        agent_semantic_content_identity::file_content_digest_v1(&binary).expect("binary digest");
    assert_eq!(
        rebind_active_asp_binary_receipt_if_present(&binary, &changed_binary_digest, &activation,)
            .expect("rebind changed ASP binary"),
        ActiveAspArtifactReconciliationV1::Updated
    );
    assert_eq!(
        rebind_active_asp_binary_receipt_if_present(&binary, &changed_binary_digest, &activation,)
            .expect("rebind current ASP binary"),
        ActiveAspArtifactReconciliationV1::Current
    );

    std::fs::remove_dir_all(root).expect("remove fixture");
}

#[test]
fn reconciliation_preserves_globally_installed_provider_leaves_outside_activation() {
    let root = fixture_root("provider-closure-shrink");
    let binary = root.join("runtime/bin/asp");
    let provider = root.join("runtime/bin/rs-harness");
    let activation = root.join("hooks/state/activation.json");
    std::fs::create_dir_all(binary.parent().expect("binary parent")).expect("create binary parent");
    std::fs::create_dir_all(activation.parent().expect("activation parent"))
        .expect("create activation parent");
    std::fs::write(&binary, b"asp-v1").expect("write binary");
    std::fs::write(&provider, b"provider-v1").expect("write provider");
    std::fs::write(
        &activation,
        br#"{"providers":[{"languageId":"rust","providerId":"rs-harness"}]}"#,
    )
    .expect("write activation");
    let binary_digest =
        agent_semantic_content_identity::file_content_digest_v1(&binary).expect("binary digest");
    let provider_digest = agent_semantic_content_identity::file_content_digest_v1(&provider)
        .expect("provider digest");
    materialize_active_asp_artifact_receipt(
        &binary,
        &binary_digest,
        &activation,
        &[ActiveAspArtifactInput {
            logical_path: "providers/rust/rs-harness".to_owned(),
            artifact_kind: ActiveArtifactKindV1::ProviderBinary,
            materialized_path: provider,
            artifact_digest: provider_digest,
        }],
    )
    .expect("materialize provider receipt");

    std::fs::write(&activation, br#"{"providers":[]}"#).expect("shrink activation closure");
    assert_eq!(
        rebind_active_asp_binary_receipt_if_present(&binary, &binary_digest, &activation)
            .expect("rebind binary with shrunken provider closure"),
        ActiveAspArtifactReconciliationV1::Updated
    );
    let receipt = verify_active_asp_artifact_receipt(&activation, &[&binary])
        .expect("verify reconciled active receipt");
    assert!(receipt.leaves().iter().any(|leaf| {
        leaf.artifact_kind() == ActiveArtifactKindV1::ProviderBinary
            && leaf.logical_path() == "providers/rust/rs-harness"
    }));

    std::fs::remove_dir_all(root).expect("remove fixture");
}
