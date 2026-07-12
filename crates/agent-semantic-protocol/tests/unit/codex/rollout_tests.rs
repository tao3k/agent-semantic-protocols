use super::rollout::{
    CodexRolloutSessionActivity, CodexRolloutSessionActivityState,
    fast_rollout_path_for_session_id, fast_rollout_path_for_session_id_in, rollout_session_meta,
};
use std::fs;
use std::path::Path;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

fn rollout_session_activity_for_test(rollout_path: &Path) -> CodexRolloutSessionActivity {
    let body = fs::read_to_string(rollout_path).expect("read rollout activity fixture");
    let mut state = CodexRolloutSessionActivityState::default();
    for line in body.lines().filter(|line| !line.trim().is_empty()) {
        let value = serde_json::from_str::<serde_json::Value>(line).expect("parse rollout jsonl");
        state.observe_event(&value, line.len() + 1);
    }
    state.finish()
}

#[test]
fn fast_rollout_lookup_uses_rollout_filename_not_content_search() {
    assert!(fast_rollout_path_for_session_id("").is_none());

    let session_id = "019f2dc6-3ed6-73b3-809d-62c4a3802ffb";
    let root = std::env::temp_dir().join(format!(
        "asp-rollout-lookup-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time")
            .as_nanos()
    ));
    let day_dir = root.join("2026/07/04");
    fs::create_dir_all(&day_dir).expect("create rollout day dir");
    let expected_path = day_dir.join(format!("rollout-2026-07-04T08-36-35-{session_id}.jsonl"));
    fs::write(
        &expected_path,
        r#"{"timestamp":"2026-07-04T15:36:35.171Z","type":"session_meta","payload":{"id":"fixture-without-target-id"}}"#,
    )
    .expect("write rollout fixture");

    let started = Instant::now();
    let actual =
        fast_rollout_path_for_session_id_in(&root, session_id).expect("fast rollout path lookup");
    let elapsed = started.elapsed();

    assert_eq!(actual, expected_path);
    assert!(
        elapsed.as_millis() < 250,
        "fast rollout lookup regressed to {:?}",
        elapsed
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
fn rollout_liveness_uses_the_filename_matched_tail_under_the_fast_path_gate() {
    let session_id = "019f2dc6-3ed6-73b3-809d-62c4a3802ffb";
    let root = std::env::temp_dir().join(format!(
        "asp-rollout-liveness-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time")
            .as_nanos()
    ));
    let day_dir = root.join("2026/07/04");
    fs::create_dir_all(&day_dir).expect("create rollout day dir");
    let rollout_path = day_dir.join(format!("rollout-2026-07-04T08-36-35-{session_id}.jsonl"));
    fs::write(
        &rollout_path,
        concat!(
            r#"{"timestamp":"2026-07-04T15:36:35.171Z","type":"session_meta","payload":{"id":"019f2dc6-3ed6-73b3-809d-62c4a3802ffb"}}"#,
            "\n",
            r#"{"timestamp":"2026-07-04T17:29:50.992Z","type":"task_complete","payload":{}}"#,
            "\n"
        ),
    )
    .expect("write rollout liveness fixture");

    let started = Instant::now();
    let liveness = super::rollout::rollout_session_liveness_for_session_id_in(&root, session_id);
    let elapsed = started.elapsed();

    match &liveness {
        super::rollout::CodexRolloutSessionLiveness::Resumable(activity)
        | super::rollout::CodexRolloutSessionLiveness::Active(activity)
        | super::rollout::CodexRolloutSessionLiveness::Unknown(activity) => {
            assert!(!activity.status.is_empty());
            assert!(activity.scanned_bytes > 0);
        }
        super::rollout::CodexRolloutSessionLiveness::Missing => {}
        super::rollout::CodexRolloutSessionLiveness::Unavailable(error) => {
            assert!(!error.is_empty());
        }
    }
    assert!(matches!(
        liveness,
        super::rollout::CodexRolloutSessionLiveness::Resumable(activity)
            if activity.last_event_kind.as_deref() == Some("task_complete")
    ));
    assert!(matches!(
        super::rollout::rollout_session_liveness_for_session_id(""),
        super::rollout::CodexRolloutSessionLiveness::Missing
    ));
    assert!(
        elapsed.as_millis() < 50,
        "fast rollout liveness regressed to {elapsed:?}"
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
fn rollout_session_meta_extracts_codex_subagent_topology() {
    let root = std::env::temp_dir().join(format!(
        "asp-rollout-meta-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time")
            .as_nanos()
    ));
    fs::create_dir_all(&root).expect("create rollout meta dir");
    let rollout_path =
        root.join("rollout-2026-07-01T23-13-41-019f2176-2f36-76c3-84e0-b3b62f367528.jsonl");
    fs::write(
        &rollout_path,
        r#"{"timestamp":"2026-07-02T06:13:43.454Z","type":"session_meta","payload":{"session_id":"019f1f1e-069d-7fc0-a816-e53fe92d3aaa","id":"019f2176-2f36-76c3-84e0-b3b62f367528","parent_thread_id":"019f1f1e-069d-7fc0-a816-e53fe92d3aaa","timestamp":"2026-07-02T06:13:41.386Z","cwd":"/Users/guangtao/ghq/github.com/tao3k/poo-flow","originator":"Codex Desktop","cli_version":"0.142.5","source":{"subagent":{"thread_spawn":{"parent_thread_id":"019f1f1e-069d-7fc0-a816-e53fe92d3aaa","depth":1,"agent_path":null,"agent_nickname":"ASP search","agent_role":"asp_explorer"}}}}}"#,
    )
    .expect("write rollout meta fixture");

    let meta = rollout_session_meta(&rollout_path).expect("rollout session meta");

    assert_eq!(
        meta.child_session_id.as_deref(),
        Some("019f2176-2f36-76c3-84e0-b3b62f367528")
    );
    assert_eq!(
        meta.source_session_id.as_deref(),
        Some("019f1f1e-069d-7fc0-a816-e53fe92d3aaa")
    );
    assert_eq!(
        meta.parent_thread_id.as_deref(),
        Some("019f1f1e-069d-7fc0-a816-e53fe92d3aaa")
    );
    assert_eq!(meta.subagent_depth, Some(1));
    assert_eq!(meta.agent_role.as_deref(), Some("asp_explorer"));
    assert_eq!(meta.relationship_kind, "subagent");

    let _ = fs::remove_dir_all(root);
}

#[test]
fn rollout_session_activity_marks_terminal_turn_as_resumable() {
    let root = std::env::temp_dir().join(format!(
        "asp-rollout-activity-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time")
            .as_nanos()
    ));
    fs::create_dir_all(&root).expect("create rollout activity dir");
    let rollout_path =
        root.join("rollout-2026-07-04T08-36-35-019f2dc6-3ed6-73b3-809d-62c4a3802ffb.jsonl");
    fs::write(
        &rollout_path,
        concat!(
            r#"{"timestamp":"2026-07-04T15:36:37.844Z","type":"session_meta","payload":{"id":"019f2dc6-3ed6-73b3-809d-62c4a3802ffb"}}"#,
            "\n",
            r#"{"timestamp":"2026-07-04T17:29:48.963Z","type":"agent_message","payload":{"text":"done"}}"#,
            "\n",
            r#"{"timestamp":"2026-07-04T17:29:50.992Z","type":"task_complete","payload":{}}"#,
            "\n"
        ),
    )
    .expect("write rollout activity fixture");

    let activity = rollout_session_activity_for_test(&rollout_path);

    assert_eq!(activity.status, "idle-resumable");
    assert_eq!(activity.last_event_kind.as_deref(), Some("task_complete"));
    assert_eq!(
        activity.last_terminal_event.as_deref(),
        Some("task_complete")
    );
    assert!(activity.turn_complete_resumable);
    assert!(!activity.turn_running_or_active);

    let _ = fs::remove_dir_all(root);
}
#[test]
fn rollout_session_activity_marks_pending_function_call_as_tool_running() {
    let root = std::env::temp_dir().join(format!(
        "asp-rollout-tool-running-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time")
            .as_nanos()
    ));
    fs::create_dir_all(&root).expect("create rollout activity dir");
    let rollout_path =
        root.join("rollout-2026-07-04T08-36-35-019f2dc6-3ed6-73b3-809d-62c4a3802ffb.jsonl");
    fs::write(
        &rollout_path,
        concat!(
            r#"{"timestamp":"2026-07-04T15:36:37.844Z","type":"session_meta","payload":{"id":"019f2dc6-3ed6-73b3-809d-62c4a3802ffb"}}"#,
            "\n",
            r#"{"timestamp":"2026-07-04T17:29:15.210Z","type":"event_msg","payload":{"type":"task_started","turn_id":"turn-1"}}"#,
            "\n",
            r#"{"timestamp":"2026-07-04T17:29:21.373Z","type":"response_item","payload":{"type":"function_call","call_id":"call-1","name":"exec_command"}}"#,
            "\n"
        ),
    )
    .expect("write rollout activity fixture");

    let activity = rollout_session_activity_for_test(&rollout_path);

    assert_eq!(activity.status, "tool-running");
    assert_eq!(activity.last_event_kind.as_deref(), Some("function_call"));
    assert_eq!(activity.pending_tool_call_count, 1);
    assert!(!activity.turn_complete_resumable);
    assert!(activity.turn_running_or_active);

    let _ = fs::remove_dir_all(root);
}
