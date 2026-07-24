use super::multi_agent_child_state_snapshot;

#[test]
fn orphaned_registry_state_overrides_historical_idle_rollout_projection() {
    assert_eq!(
        multi_agent_child_state_snapshot(Some("orphan-risk"), false, None),
        "control-plane-orphaned-unbound"
    );
    assert_eq!(
        multi_agent_child_state_snapshot(Some("active"), false, None),
        "not-routable"
    );
}
