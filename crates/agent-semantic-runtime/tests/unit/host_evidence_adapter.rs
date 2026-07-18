use crate::codex_app_server_sessions::CodexHostEvidenceAdapter;

#[test]
fn rollout_only_adapter_never_starts_a_live_runtime_probe() {
    assert!(
        CodexHostEvidenceAdapter::RolloutOnly
            .read_thread_runtime_observation("019f0000-0000-7000-8000-000000000000")
            .is_none()
    );
}

#[test]
fn fixture_adapter_supplies_live_tree_with_unobservable_reasoning() {
    let child_session_id = "019f0000-0000-7000-8000-000000000101";
    let path = std::env::temp_dir().join(format!(
        "asp-host-evidence-{}-{}.json",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system time")
            .as_nanos()
    ));
    std::fs::write(
        &path,
        serde_json::to_vec(&serde_json::json!({
            "threads": [{
                "id": child_session_id,
                "parentThreadId": "019f0000-0000-7000-8000-000000000001"
            }],
            "runtime": {
                (child_session_id): { "model": "gpt-5.4-mini" }
            }
        }))
        .expect("serialize fixture"),
    )
    .expect("write fixture");

    let adapter = CodexHostEvidenceAdapter::Fixture(path.clone());
    let threads = adapter
        .read_direct_child_threads("019f0000-0000-7000-8000-000000000001")
        .expect("fixture threads");
    assert_eq!(threads.len(), 1);
    let runtime = adapter
        .read_thread_runtime_observation(child_session_id)
        .expect("fixture runtime");
    assert_eq!(runtime.model.as_deref(), Some("gpt-5.4-mini"));
    assert!(runtime.reasoning_effort.is_none());

    std::fs::remove_file(path).expect("remove fixture");
}
