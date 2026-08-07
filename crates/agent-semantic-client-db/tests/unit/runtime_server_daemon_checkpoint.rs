use super::{RUNTIME_SERVER_DAEMON_INACTIVITY_LEASE, daemon_checkpoint_should_shutdown};

#[test]
fn zero_workspace_keeps_daemon_alive_until_confirmed_missing() {
    let started = tokio::time::Instant::now();
    assert!(!daemon_checkpoint_should_shutdown(
        0,
        false,
        false,
        started,
        started + RUNTIME_SERVER_DAEMON_INACTIVITY_LEASE
    ));
    assert!(daemon_checkpoint_should_shutdown(
        0, false, true, started, started
    ));
}

#[test]
fn activity_and_idle_lease_bound_shutdown() {
    let started = tokio::time::Instant::now();
    assert!(!daemon_checkpoint_should_shutdown(
        0,
        true,
        false,
        started,
        started + RUNTIME_SERVER_DAEMON_INACTIVITY_LEASE - std::time::Duration::from_secs(1)
    ));
    let refreshed = started + std::time::Duration::from_secs(10);
    assert!(!daemon_checkpoint_should_shutdown(
        0,
        true,
        false,
        refreshed,
        refreshed + RUNTIME_SERVER_DAEMON_INACTIVITY_LEASE - std::time::Duration::from_secs(1)
    ));
    assert!(daemon_checkpoint_should_shutdown(
        0,
        true,
        false,
        refreshed,
        refreshed + RUNTIME_SERVER_DAEMON_INACTIVITY_LEASE
    ));
}
