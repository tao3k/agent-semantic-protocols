#[path = "../../../src/command/workspace_db_checkpoint.rs"]
mod subject;

use subject::{ResidentServiceCheckpointDecision, checkpoint_decision};

#[test]
fn existing_active_workspace_continues() {
    assert_eq!(
        checkpoint_decision(true, 1_000, 1_100, std::time::Duration::from_secs(3_600),),
        ResidentServiceCheckpointDecision::Continue
    );
}

#[test]
fn missing_workspace_shuts_down_immediately() {
    assert_eq!(
        checkpoint_decision(false, 1_000, 1_001, std::time::Duration::from_secs(3_600),),
        ResidentServiceCheckpointDecision::ShutdownWorkspaceMissing
    );
}

#[test]
fn one_hour_idle_workspace_shuts_down() {
    assert_eq!(
        checkpoint_decision(true, 1_000, 4_600, std::time::Duration::from_secs(3_600),),
        ResidentServiceCheckpointDecision::ShutdownIdle
    );
}
