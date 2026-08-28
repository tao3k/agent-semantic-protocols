use super::{dispatch_execution_context_allowed, validate_exact_argv};

#[test]
fn resident_bridge_accepts_parser_and_testing_argv_without_lane_coupling() {
    assert!(validate_exact_argv(&["asp".into(), "rust".into(), "search".into()]).is_ok());
    assert!(validate_exact_argv(&["cargo".into(), "test".into(), "--workspace".into()]).is_ok());
}

#[test]
fn resident_bridge_rejects_recursive_dispatch() {
    assert!(
        validate_exact_argv(&[
            "asp".into(),
            "agent".into(),
            "session".into(),
            "dispatch-execute".into(),
        ])
        .is_err()
    );
}

#[test]
fn root_execution_requires_a_claimed_canonical_resident_bridge() {
    assert!(dispatch_execution_context_allowed(
        "root",
        "root",
        "resident-command-bridge:/root/asp_testing",
    ));
    assert!(!dispatch_execution_context_allowed(
        "root",
        "root",
        "/root/asp_testing",
    ));
    assert!(dispatch_execution_context_allowed(
        "child",
        "root",
        "resident-command-bridge:/root/asp_testing",
    ));
}
