use super::{CodexRolloutSessionLiveness, rollout_session_liveness_for_session_id_in};

#[test]
fn explicit_rollout_root_reports_missing_session() {
    let root = std::env::temp_dir().join("asp-rollout-missing-session");
    assert!(matches!(
        rollout_session_liveness_for_session_id_in(&root, "missing-session"),
        CodexRolloutSessionLiveness::Missing
    ));
}
