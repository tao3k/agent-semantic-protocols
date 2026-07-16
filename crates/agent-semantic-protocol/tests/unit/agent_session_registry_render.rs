use super::status_has_trusted_live_binding;

#[test]
fn only_trusted_ready_binding_can_replace_unknown_host_status() {
    assert!(status_has_trusted_live_binding(
        Some("ready"),
        Some("trusted-live-identity-binding")
    ));
    assert!(!status_has_trusted_live_binding(
        Some("ready"),
        Some("persisted-message-target-without-live-attestation")
    ));
    assert!(!status_has_trusted_live_binding(
        Some("unbound"),
        Some("trusted-live-identity-binding")
    ));
}
