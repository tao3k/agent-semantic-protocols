use super::provider_native_exact_fallback_reason;

#[test]
fn stable_ipc_and_generation_open_failures_admit_provider_native_exact() {
    for (error, expected) in [
        (
            "failed to connect Runtime Server data endpoint reasonKind=host-local-ipc-permission-denied errorKind=permission-denied",
            "host-local-ipc-permission-denied",
        ),
        (
            "exact source query state=source-unavailable reasonKind=active-workspace-generation-required",
            "active-workspace-generation-required",
        ),
        (
            "canonical workspace scope is not admitted: projectRoot=/repo",
            "active-workspace-generation-required",
        ),
        (
            "failed to connect Runtime Server data endpoint: connection refused",
            "runtime-server-stable-ipc-unavailable",
        ),
    ] {
        assert_eq!(provider_native_exact_fallback_reason(error), Some(expected));
    }
}

#[test]
fn identity_drift_and_corruption_never_fall_back() {
    for error in [
        "runtime-server-data-binding-mismatch: stale epoch",
        "authority-corrupt: invalid search authority",
        "runtime-server-selector-read-failed: selector identity mismatch",
    ] {
        assert_eq!(provider_native_exact_fallback_reason(error), None);
    }
}
