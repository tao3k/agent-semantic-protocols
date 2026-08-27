use super::normalize_server_shutdown_for_identity_handoff;

#[test]
fn monitor_requested_accept_loop_shutdown_is_a_clean_drain() {
    assert!(
        normalize_server_shutdown_for_identity_handoff(
            true,
            Err("serve HTTP/2 JSON connection: connection error".to_owned()),
        )
        .is_ok()
    );
}

#[test]
fn spontaneous_server_failure_remains_terminal() {
    assert!(
        normalize_server_shutdown_for_identity_handoff(
            false,
            Err("serve HTTP/2 JSON connection: connection error".to_owned()),
        )
        .is_err()
    );
}
