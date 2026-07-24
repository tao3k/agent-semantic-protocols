use agent_semantic_rust_policy_verifier::verify_receipt_bytes;

use super::fixtures::{member, registry, source_snapshot, verification_input};

#[test]
fn duplicate_registry_member_is_rejected() {
    let mut registry = registry();
    registry.members.push(member());
    let snapshot = source_snapshot();
    let error = verify_receipt_bytes(
        &serde_json::to_vec(&registry).unwrap(),
        b"{}",
        verification_input(&snapshot),
    )
    .unwrap_err();
    assert!(
        error.contains("duplicate Rust member policy"),
        "error={error}"
    );
}
