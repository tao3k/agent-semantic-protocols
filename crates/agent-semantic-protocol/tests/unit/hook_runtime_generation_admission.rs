use super::hook_event_requires_generation_admission;

#[test]
fn workspace_lifecycle_events_admit_generation() {
    for event in ["session-start", "user-prompt"] {
        assert!(hook_event_requires_generation_admission(&[
            "--event".to_owned(),
            event.to_owned(),
        ]));
    }
}

#[test]
fn tool_events_do_not_add_runtime_ipc_to_the_matcher_hot_path() {
    for event in ["pre-tool", "permission-request", "post-tool", "stop"] {
        assert!(!hook_event_requires_generation_admission(&[
            "--event".to_owned(),
            event.to_owned(),
        ]));
    }
}
