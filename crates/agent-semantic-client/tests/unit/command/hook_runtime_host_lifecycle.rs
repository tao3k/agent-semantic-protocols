// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use super::{AgentHostLifecycleEventIpc, publish_host_lifecycle_event_locally};
use agent_semantic_client_db::workspace_db_ipc::AgentHostLifecycleEventKind;
use serde_json::Value;

fn event(id: &str) -> AgentHostLifecycleEventIpc {
    AgentHostLifecycleEventIpc {
        host_event_id: id.to_owned().into(),
        host_event_sequence: 0,
        namespace_id: "blake3-256:test-namespace".to_owned().into(),
        kind: AgentHostLifecycleEventKind::Started,
        platform: "codex".to_owned(),
        project_id: "test-project".into(),
        root_session_id: "root".into(),
        parent_session_id: "root".into(),
        child_session_id: "child".into(),
        host_task_name: "asp_explorer".to_owned(),
        platform_host_agent_name: "asp_explorer".to_owned(),
        route_key: "asp-explore".to_owned().into(),
        profile_id: "profiles/asp-explore.toml".to_owned().into(),
        role: "explorer".to_owned(),
        model: "test-model".to_owned(),
        model_digest: "blake3-256:model".to_owned(),
        profile_digest: "blake3-256:profile".to_owned(),
        sandbox_mode: agent_semantic_client_db::workspace_db_ipc::AgentHostSandboxMode::ReadOnly,
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
            .join("hooks/host-sessions/test-namespace/authority.json")
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
        tokio::fs::read(state_home.join("hooks/host-sessions/test-namespace/authority.json"))
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

#[tokio::test]
async fn publication_lock_contention_is_bounded() {
    let state_home = isolated_state_home("bounded-lock-contention");
    let authority_dir = state_home.join("hooks/host-sessions/test-namespace");
    tokio::fs::create_dir_all(&authority_dir)
        .await
        .expect("create publication authority");
    let lock_file = std::fs::OpenOptions::new()
        .create(true)
        .read(true)
        .write(true)
        .open(authority_dir.join(".publication-lock"))
        .expect("open contended publication lock");
    fs2::FileExt::lock_exclusive(&lock_file).expect("hold contended publication lock");
    let mut lifecycle_event = event("blake3-256:contended");
    let started = std::time::Instant::now();

    let error = tokio::time::timeout(
        std::time::Duration::from_millis(500),
        publish_host_lifecycle_event_locally(&state_home, &mut lifecycle_event),
    )
    .await
    .expect("local publication lock acquisition must be bounded")
    .expect_err("a live publication lock must fail closed");

    assert!(error.contains("timed out acquiring Host lifecycle authority lock"));
    assert!(started.elapsed() < std::time::Duration::from_millis(500));
    fs2::FileExt::unlock(&lock_file).expect("release contended publication lock");
    let _ = tokio::fs::remove_dir_all(state_home).await;
}

#[tokio::test]
async fn failed_publication_releases_its_lock() {
    let state_home = isolated_state_home("failed-publication-cleanup");
    let authority_dir = state_home.join("hooks/host-sessions/test-namespace");
    tokio::fs::create_dir_all(&authority_dir)
        .await
        .expect("create authority directory");
    tokio::fs::write(authority_dir.join("authority.json"), b"not-json")
        .await
        .expect("write corrupt authority fixture");
    let mut lifecycle_event = event("blake3-256:decode-failure");

    let error = publish_host_lifecycle_event_locally(&state_home, &mut lifecycle_event)
        .await
        .expect_err("corrupt authority must fail closed");

    assert!(error.contains("failed to decode Host lifecycle authority"));
    let lock_file = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(authority_dir.join(".publication-lock"))
        .expect("reopen persistent publication lock file");
    fs2::FileExt::try_lock_exclusive(&lock_file)
        .expect("publication failure must release its advisory lock");
    fs2::FileExt::unlock(&lock_file).expect("release verification lock");
    let _ = tokio::fs::remove_dir_all(state_home).await;
}

#[tokio::test]
async fn concurrent_publications_complete_with_one_monotonic_sequence() {
    let state_home = isolated_state_home("concurrent-publication");
    let mut first = event("blake3-256:concurrent-first");
    let mut second = event("blake3-256:concurrent-second");

    let (first_result, second_result) = tokio::join!(
        publish_host_lifecycle_event_locally(&state_home, &mut first),
        publish_host_lifecycle_event_locally(&state_home, &mut second),
    );
    first_result.expect("first concurrent publication");
    second_result.expect("second concurrent publication");

    let mut sequences = [first.host_event_sequence, second.host_event_sequence];
    sequences.sort_unstable();
    assert_eq!(sequences, [1, 2]);
    let _ = tokio::fs::remove_dir_all(state_home).await;
}
