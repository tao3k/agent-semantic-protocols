use agent_semantic_rust_policy_types::SourceSnapshot;
use agent_semantic_rust_policy_verifier::verify_receipt_bytes;

use super::fixtures::{receipt, registry, source_snapshot, verification_input};

#[test]
fn malformed_registry_fails_closed_before_receipt_acceptance() {
    let snapshot = source_snapshot();
    let error = verify_receipt_bytes(b"{}", b"{}", verification_input(&snapshot)).unwrap_err();
    assert!(error.contains("registry"));
}

#[test]
fn valid_receipt_is_accepted_and_source_drift_is_rejected() {
    let registry = registry();
    let receipt = receipt(&registry);
    let registry_bytes = serde_json::to_vec(&registry).unwrap();
    let receipt_bytes = serde_json::to_vec(&receipt).unwrap();
    let snapshot = source_snapshot();

    let verified = verify_receipt_bytes(
        &registry_bytes,
        &receipt_bytes,
        verification_input(&snapshot),
    )
    .unwrap();
    assert_eq!(verified.package_name, "demo");
    assert_eq!(verified.package_directory, "crates/demo");

    let drifted = SourceSnapshot {
        digest: format!("blake3:{}", "1".repeat(64)),
        ..snapshot
    };
    let error = verify_receipt_bytes(
        &registry_bytes,
        &receipt_bytes,
        verification_input(&drifted),
    )
    .unwrap_err();
    assert!(error.contains("source snapshot"));
}

#[test]
fn policy_and_execution_drift_fail_closed() {
    let registry = registry();
    let registry_bytes = serde_json::to_vec(&registry).unwrap();
    let snapshot = source_snapshot();

    let mut policy_drift = receipt(&registry);
    policy_drift.policy_digest = format!("blake3:{}", "3".repeat(64));
    let error = verify_receipt_bytes(
        &registry_bytes,
        &serde_json::to_vec(&policy_drift).unwrap(),
        verification_input(&snapshot),
    )
    .unwrap_err();
    assert!(error.contains("policy drift"), "error={error}");

    let mut execution_drift = receipt(&registry);
    execution_drift.execution_command_digest = format!("blake3:{}", "4".repeat(64));
    let error = verify_receipt_bytes(
        &registry_bytes,
        &serde_json::to_vec(&execution_drift).unwrap(),
        verification_input(&snapshot),
    )
    .unwrap_err();
    assert!(error.contains("execution-command drift"), "error={error}");
}
