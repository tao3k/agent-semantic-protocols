#[test]
fn verified_canonical_target_presence_is_independent_of_model_evidence() {
    // Model evidence is deliberately not an input to host-target identity.
    // Runtime-profile admission remains a separate bootstrap/dispatch gate.
    assert!(super::verified_canonical_host_target_present(true));
    assert!(!super::verified_canonical_host_target_present(false));
}
