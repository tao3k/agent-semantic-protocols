use std::collections::BTreeSet;
use std::time::{SystemTime, UNIX_EPOCH};

use super::reconcile_registered_provider_runtime_binaries;

#[test]
fn provider_runtime_publication_enumerates_and_deduplicates_the_v1_registry() {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    let root = std::env::temp_dir().join(format!(
        "asp-provider-runtime-publication-{}-{nonce}",
        std::process::id()
    ));
    let runtime_bin = root.join("runtime/bin");
    let artifact_root = root.join("runtime/artifacts");
    let provider_lock_dir = root.join("runtime/providers/receipts");
    std::fs::create_dir_all(&runtime_bin).expect("create runtime bin");

    let registrations = agent_semantic_hook::registered_provider_binaries_v1();
    let binary_names = registrations
        .iter()
        .map(|registration| registration.binary().to_string())
        .collect::<BTreeSet<_>>();
    let installed = binary_names.iter().take(2).cloned().collect::<Vec<_>>();
    assert_eq!(
        installed.len(),
        2,
        "registry must expose fixture identities"
    );
    for binary_name in &installed {
        std::fs::write(
            runtime_bin.join(binary_name),
            format!("provider-binary:{binary_name}"),
        )
        .expect("write provider fixture");
    }

    let receipt = reconcile_registered_provider_runtime_binaries(
        &runtime_bin,
        &artifact_root,
        &provider_lock_dir,
    )
    .expect("reconcile registered provider binaries");

    assert_eq!(receipt.registration_count, registrations.len());
    assert_eq!(receipt.binary_identity_count, binary_names.len());
    assert_eq!(receipt.reconciled_count, installed.len());
    assert_eq!(receipt.changed_count, installed.len());
    assert_eq!(
        receipt.missing_count,
        binary_names.len().saturating_sub(installed.len())
    );
    assert_eq!(receipt.receipt_reconciled_count, 0);
    for binary_name in installed {
        assert_eq!(
            std::fs::read_link(runtime_bin.join(&binary_name)).expect("stable provider link"),
            std::path::PathBuf::from("../artifacts/blake3-256/latest").join(&binary_name)
        );
        assert!(
            artifact_root
                .join("blake3-256/latest")
                .join(binary_name)
                .exists()
        );
    }

    let idempotent = reconcile_registered_provider_runtime_binaries(
        &runtime_bin,
        &artifact_root,
        &provider_lock_dir,
    )
    .expect("reconcile current registered provider binaries");
    assert_eq!(idempotent.reconciled_count, 2);
    assert_eq!(idempotent.changed_count, 0);

    std::fs::remove_dir_all(root).expect("remove fixture");
}
