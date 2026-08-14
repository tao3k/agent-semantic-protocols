use super::{AgentHostLifecycleEventIpc, publish_host_lifecycle_event_locally};
use agent_semantic_client_db::workspace_db_ipc::AgentHostLifecycleEventKind;
use serde_json::Value;

fn event(id: &str) -> AgentHostLifecycleEventIpc {
    AgentHostLifecycleEventIpc {
        host_event_id: id.to_owned(),
        host_event_sequence: 0,
        namespace_id: "blake3-256:test-namespace".to_owned(),
        kind: AgentHostLifecycleEventKind::Started,
        platform: "codex".to_owned(),
        project_id: "test-project".to_owned(),
        root_session_id: "root".to_owned(),
        parent_session_id: "root".to_owned(),
        child_session_id: "child".to_owned(),
        host_task_name: "asp_explorer".to_owned(),
        platform_host_agent_name: "asp_explorer".to_owned(),
        route_key: "asp-explore".to_owned(),
        profile_id: "profiles/asp-explore.toml".to_owned(),
        role: "explorer".to_owned(),
        model: "test-model".to_owned(),
        model_digest: "blake3-256:model".to_owned(),
        profile_digest: "blake3-256:profile".to_owned(),
        sandbox_mode: "read-only".to_owned(),
        session_lifetime: "persistent".to_owned(),
        payload_digest: "blake3-256:payload".to_owned(),
        transcript_path: None,
        observed_at: 1,
    }
}

fn isolated_state_home(test_name: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!(
        "asp-host-lifecycle-{test_name}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("test clock must follow Unix epoch")
            .as_nanos()
    ))
}

#[tokio::test]
async fn local_publication_does_not_require_runtime() {
    let state_home = isolated_state_home("runtime-unavailable");
    let mut lifecycle_event = event("blake3-256:first");

    publish_host_lifecycle_event_locally(&state_home, &mut lifecycle_event)
        .await
        .expect("local Host authority must publish without a Runtime endpoint");

    assert_eq!(lifecycle_event.host_event_sequence, 1);
    assert!(
        state_home
            .join("hooks/host-sessions/test-namespace/authority.v1.json")
            .is_file()
    );
    let _ = tokio::fs::remove_dir_all(state_home).await;
}

#[tokio::test]
async fn duplicate_publication_is_idempotent() {
    let state_home = isolated_state_home("duplicate");
    let mut first = event("blake3-256:same");
    publish_host_lifecycle_event_locally(&state_home, &mut first)
        .await
        .expect("first publication must succeed");
    let mut duplicate = event("blake3-256:same");
    publish_host_lifecycle_event_locally(&state_home, &mut duplicate)
        .await
        .expect("duplicate publication must succeed");

    assert_eq!(first.host_event_sequence, 1);
    assert_eq!(duplicate.host_event_sequence, 1);
    let authority =
        tokio::fs::read(state_home.join("hooks/host-sessions/test-namespace/authority.v1.json"))
            .await
            .expect("authority must be readable");
    let authority: Value = serde_json::from_slice(&authority).expect("authority must be JSON");
    assert_eq!(authority["lastSequence"], serde_json::json!(1));
    assert_eq!(authority["events"].as_array().map(Vec::len), Some(1));
    let _ = tokio::fs::remove_dir_all(state_home).await;
}

#[tokio::test]
async fn publication_sequence_is_monotonic() {
    let state_home = isolated_state_home("sequence");
    let mut first = event("blake3-256:first");
    let mut second = event("blake3-256:second");
    publish_host_lifecycle_event_locally(&state_home, &mut first)
        .await
        .expect("first publication must succeed");
    publish_host_lifecycle_event_locally(&state_home, &mut second)
        .await
        .expect("second publication must succeed");

    assert_eq!(first.host_event_sequence, 1);
    assert_eq!(second.host_event_sequence, 2);
    let _ = tokio::fs::remove_dir_all(state_home).await;
}
