use crate::runtime_identity_monitor::{MonitorAction, RuntimeIdentityMonitor};

#[test]
fn same_identity_is_a_noop_after_ready() {
    let mut monitor = RuntimeIdentityMonitor::default();
    assert_eq!(monitor.observe("a"), MonitorAction::BeginDrain);
    assert_eq!(monitor.drain_completed(), MonitorAction::SpawnLatest);
    assert_eq!(monitor.observe("a"), MonitorAction::Noop);
}

#[test]
fn latest_identity_wins_during_drain() {
    let mut monitor = RuntimeIdentityMonitor::default();
    assert_eq!(monitor.observe("a"), MonitorAction::BeginDrain);
    assert_eq!(monitor.observe("b"), MonitorAction::Noop);
    assert_eq!(monitor.observe("c"), MonitorAction::Noop);
    assert_eq!(monitor.drain_completed(), MonitorAction::SpawnLatest);
}
