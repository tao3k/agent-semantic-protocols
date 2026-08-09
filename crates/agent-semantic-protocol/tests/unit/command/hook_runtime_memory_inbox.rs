use std::time::{Duration, Instant};

use agent_semantic_client_db::workspace_db_ipc::{
    AgentHostLifecycleEventIpc, AgentHostLifecycleEventKind,
};

fn lifecycle_event(identity: &str) -> AgentHostLifecycleEventIpc {
    AgentHostLifecycleEventIpc {
        host_event_id: identity.to_owned(),
        host_event_sequence: 0,
        namespace_id: "namespace-1".to_owned(),
        kind: AgentHostLifecycleEventKind::Started,
        platform: "codex".to_owned(),
        project_id: "workspace-1".to_owned(),
        root_session_id: "root-1".to_owned(),
        parent_session_id: "root-1".to_owned(),
        child_session_id: "child-1".to_owned(),
        host_task_name: "asp_explorer".to_owned(),
        platform_host_agent_name: "asp_explorer".to_owned(),
        route_key: "asp_explorer".to_owned(),
        profile_id: "agents/asp_explorer.toml".to_owned(),
        role: "explore".to_owned(),
        model: "gpt-test".to_owned(),
        model_digest: "blake3-256:model".to_owned(),
        profile_digest: "blake3-256:profile".to_owned(),
        sandbox_mode: "read-only".to_owned(),
        session_lifetime: "resident".to_owned(),
        payload_digest: "blake3-256:payload".to_owned(),
        transcript_path: None,
        observed_at: 10,
    }
}

#[tokio::test(flavor = "current_thread")]
async fn mmap_inbox_is_runtime_independent_bounded_and_idempotent() {
    let state_dir = tempfile::tempdir().expect("Hook mmap state dir");
    let started = Instant::now();
    let sequence = super::append_host_lifecycle_event_in_state_dir(
        state_dir.path(),
        lifecycle_event("event-1"),
    )
    .await
    .expect("append without Runtime Server endpoint");
    assert_eq!(sequence, 1);
    assert!(
        started.elapsed() < Duration::from_millis(100),
        "cold mmap append exceeded Hook budget: {:?}",
        started.elapsed()
    );

    let duplicate = super::append_host_lifecycle_event_in_state_dir(
        state_dir.path(),
        lifecycle_event("event-1"),
    )
    .await
    .expect("deduplicate replayed Host event");
    assert_eq!(duplicate, sequence);

    let pending = super::pending_hook_events_in_state_dir(state_dir.path())
        .await
        .expect("read mmap inbox");
    assert_eq!(pending.len(), 1);
    assert_eq!(pending[0].inbox_sequence, 1);
    assert_eq!(pending[0].event["hostEventSequence"], 1);

    super::acknowledge_hook_event_in_state_dir(state_dir.path(), sequence)
        .await
        .expect("advance mmap acknowledgement watermark");
    assert!(
        super::pending_hook_events_in_state_dir(state_dir.path())
            .await
            .expect("read acknowledged mmap inbox")
            .is_empty()
    );
}

#[tokio::test(flavor = "current_thread")]
async fn mmap_inbox_preserves_monotonic_sequence_after_acknowledgement() {
    let state_dir = tempfile::tempdir().expect("Hook mmap state dir");
    let first = super::append_host_lifecycle_event_in_state_dir(
        state_dir.path(),
        lifecycle_event("event-1"),
    )
    .await
    .expect("append first event");
    super::acknowledge_hook_event_in_state_dir(state_dir.path(), first)
        .await
        .expect("acknowledge first event");
    let second = super::append_host_lifecycle_event_in_state_dir(
        state_dir.path(),
        lifecycle_event("event-2"),
    )
    .await
    .expect("append second event");
    assert_eq!(second, first + 1);
}

#[tokio::test(flavor = "current_thread")]
async fn all_one_shot_hook_events_share_the_local_mmap_plane() {
    let state_dir = tempfile::tempdir().expect("Hook mmap state dir");

    super::append_workspace_mutation_in_state_dir(
        state_dir.path(),
        "mutation-1",
        vec!["src/lib.rs".to_owned()],
    )
    .await
    .expect("append workspace mutation without Runtime");
    super::append_runtime_performance_observation_in_state_dir(
        state_dir.path(),
        serde_json::json!({"schemaId": "wall-failure-fixture"}),
    )
    .await
    .expect("append performance observation without Runtime");

    let kinds = super::pending_hook_events_in_state_dir(state_dir.path())
        .await
        .expect("read all-event mmap inbox")
        .into_iter()
        .map(|record| record.entry_kind)
        .collect::<Vec<_>>();
    assert_eq!(
        kinds,
        ["workspace-mutation", "runtime-performance-observation"]
    );
}

#[test]
fn hook_execution_plane_has_no_runtime_server_dependency() {
    let command_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("src")
        .join("command");
    for owner in [
        "hook_runtime.rs",
        "hook_runtime_host_lifecycle.rs",
        "hook_runtime_memory_inbox.rs",
        "hook_runtime_workspace_mutation.rs",
        "hook_runtime_performance_failure.rs",
    ] {
        let source = std::fs::read_to_string(command_dir.join(owner)).expect("read Hook owner");
        for forbidden in [
            "crate::server",
            "ensure_runtime_server",
            "connect_runtime_server",
            "call_runtime_server",
            "RuntimeServerClientExecutor",
            "admit_to_runtime",
        ] {
            assert!(
                !source.contains(forbidden),
                "Hook owner {owner} regained forbidden Runtime dependency `{forbidden}`"
            );
        }
    }
    let supervisor = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("src/server/runtime_server_supervisor.rs"),
    )
    .expect("read Runtime supervisor owner");
    assert!(!supervisor.contains("ensure_runtime_server_for_hook"));
}
