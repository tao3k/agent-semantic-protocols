#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum ResidentServiceCheckpointDecision {
    Continue,
    ShutdownIdle,
    ShutdownWorkspaceMissing,
}

pub(super) fn checkpoint_decision(
    workspace_exists: bool,
    last_activity_epoch_seconds: u64,
    now_epoch_seconds: u64,
    idle_timeout: std::time::Duration,
) -> ResidentServiceCheckpointDecision {
    if !workspace_exists {
        return ResidentServiceCheckpointDecision::ShutdownWorkspaceMissing;
    }
    if now_epoch_seconds.saturating_sub(last_activity_epoch_seconds) >= idle_timeout.as_secs() {
        return ResidentServiceCheckpointDecision::ShutdownIdle;
    }
    ResidentServiceCheckpointDecision::Continue
}
