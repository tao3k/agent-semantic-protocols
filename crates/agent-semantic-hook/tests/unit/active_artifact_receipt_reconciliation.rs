use std::time::{SystemTime, UNIX_EPOCH};

use super::{
    ActiveAspArtifactReconciliation, atomic_write_compare_exchange,
    materialize_active_asp_artifact_receipt, rebind_active_asp_binary_receipt_if_present,
    verify_active_asp_artifact_receipt,
};
use crate::{registered_language_ids, registered_provider_id};
use agent_semantic_content_identity::active_artifact_merkle::{
    ActiveArtifactKind, ActiveAspArtifactReceipt,
};

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
        ActiveAspArtifactReconciliation::NotMaterialized
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
    materialize_active_asp_artifact_receipt(&binary, &binary_digest, &activation)
        .expect("materialize receipt");

    std::fs::write(&activation, b"{\"generation\":2,\"rankers\":[]}").expect("update activation");

    std::fs::write(&binary, b"asp-v2-with-a-new-size").expect("update binary");
    let changed_binary_digest =
        agent_semantic_content_identity::file_content_digest_v1(&binary).expect("binary digest");
    assert_eq!(
        rebind_active_asp_binary_receipt_if_present(&binary, &changed_binary_digest, &activation,)
            .expect("rebind changed ASP binary"),
        ActiveAspArtifactReconciliation::Updated
    );
    assert_eq!(
        rebind_active_asp_binary_receipt_if_present(&binary, &changed_binary_digest, &activation,)
            .expect("rebind current ASP binary"),
        ActiveAspArtifactReconciliation::Current
    );

    std::fs::remove_dir_all(root).expect("remove fixture");
}

#[test]
fn asp_binary_rebind_drops_provider_leaves_outside_its_authority() {
    let root = fixture_root("provider-closure-shrink");
    let binary = root.join("runtime/bin/asp");
    let activation = root.join("hooks/state/activation.json");
    std::fs::create_dir_all(binary.parent().expect("binary parent")).expect("create binary parent");
    std::fs::create_dir_all(activation.parent().expect("activation parent"))
        .expect("create activation parent");
    std::fs::write(&binary, b"asp-v1").expect("write binary");
    std::fs::write(
        &activation,
        br#"{"providers":[{"languageId":"rust","providerId":"rs-harness"}]}"#,
    )
    .expect("write activation");
    let binary_digest =
        agent_semantic_content_identity::file_content_digest_v1(&binary).expect("binary digest");
    materialize_active_asp_artifact_receipt(&binary, &binary_digest, &activation)
        .expect("materialize provider receipt");

    std::fs::write(&activation, br#"{"providers":[]}"#).expect("shrink activation closure");
    assert_eq!(
        rebind_active_asp_binary_receipt_if_present(&binary, &binary_digest, &activation)
            .expect("rebind binary with shrunken provider closure"),
        ActiveAspArtifactReconciliation::Updated
    );
    let receipt = verify_active_asp_artifact_receipt(&activation, &[&binary])
        .expect("verify reconciled active receipt");
    assert!(
        receipt
            .leaves()
            .iter()
            .all(|leaf| { !matches!(leaf.artifact_kind(), ActiveArtifactKind::ProviderBinary) })
    );

    std::fs::remove_dir_all(root).expect("remove fixture");
}

#[test]
fn asp_binary_rebind_does_not_inherit_registered_provider_identities() {
    let root = fixture_root("registered-provider-recovery");
    let binary = root.join("runtime/bin/asp");
    let activation = root.join("hooks/state/activation.json");
    std::fs::create_dir_all(binary.parent().expect("binary parent")).expect("create binary parent");
    std::fs::create_dir_all(activation.parent().expect("activation parent"))
        .expect("create activation parent");
    std::fs::write(&binary, b"asp-v1").expect("write binary");

    let selected = ["gerbil-scheme", "python"]
        .into_iter()
        .map(|language_id| {
            let registered_language = registered_language_ids()
                .into_iter()
                .find(|registered| registered.as_str() == language_id)
                .unwrap_or_else(|| panic!("registered language `{language_id}`"));
            let provider_id = registered_provider_id(language_id)
                .unwrap_or_else(|| panic!("registered provider for `{language_id}`"));
            (registered_language, provider_id)
        })
        .collect::<Vec<_>>();
    let providers = selected
        .iter()
        .map(|(language_id, provider_id)| {
            serde_json::json!({
                "languageId": language_id.as_str(),
                "providerId": provider_id,
            })
        })
        .collect::<Vec<_>>();
    std::fs::write(
        &activation,
        serde_json::to_vec(&serde_json::json!({ "providers": providers }))
            .expect("encode activation"),
    )
    .expect("write activation");

    let binary_digest =
        agent_semantic_content_identity::file_content_digest_v1(&binary).expect("binary digest");
    let materialized =
        materialize_active_asp_artifact_receipt(&binary, &binary_digest, &activation)
            .expect("materialize registered provider receipt");

    std::fs::write(&binary, b"asp-v2").expect("update binary");
    let changed_binary_digest =
        agent_semantic_content_identity::file_content_digest_v1(&binary).expect("binary digest");
    assert_eq!(
        rebind_active_asp_binary_receipt_if_present(&binary, &changed_binary_digest, &activation,)
            .expect("rebind ASP while registered providers are missing"),
        ActiveAspArtifactReconciliation::Updated
    );
    let receipt: ActiveAspArtifactReceipt = serde_json::from_slice(
        &std::fs::read(&materialized.receipt_path).expect("read rebound receipt"),
    )
    .expect("decode rebound receipt");
    for (language_id, provider_id) in selected {
        let logical_path = format!("providers/{}/{}", language_id, provider_id);
        assert!(
            receipt
                .leaves()
                .iter()
                .all(|leaf| leaf.logical_path() != logical_path),
            "do not inherit `{logical_path}`"
        );
    }
    assert!(
        verify_active_asp_artifact_receipt(&activation, &[&binary]).is_ok(),
        "ASP receipt verification must not own provider materialization"
    );

    std::fs::remove_dir_all(root).expect("remove fixture");
}
