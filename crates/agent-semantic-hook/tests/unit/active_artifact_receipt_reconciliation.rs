use std::time::{SystemTime, UNIX_EPOCH};

use super::{
    ActiveAspArtifactReconciliationV1, materialize_active_asp_artifact_receipt,
    rebind_active_asp_binary_receipt_if_present, reconcile_active_asp_artifact_receipt_if_present,
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
