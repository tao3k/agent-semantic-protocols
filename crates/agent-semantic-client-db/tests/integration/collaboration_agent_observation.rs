use std::sync::Arc;

use agent_semantic_client_db::{AgentSessionRegistry, run_collaboration_snapshot_inbox};
use agent_semantic_config::{
    COLLABORATION_LIVE_AGENT_SNAPSHOT_SCHEMA_ID, CollaborationLiveAgent,
    CollaborationLiveAgentSnapshot, CollaborationLiveAgents,
};
use serde_json::json;

fn snapshot(
    observed_at_unix_ms: u64,
    agents: Vec<CollaborationLiveAgent>,
) -> CollaborationLiveAgentSnapshot {
    CollaborationLiveAgentSnapshot {
        schema_id: COLLABORATION_LIVE_AGENT_SNAPSHOT_SCHEMA_ID.to_owned(),
        schema_version: "1".to_owned(),
        workspace_root: "/workspace/project".to_owned(),
        root_session_id: "root-session".to_owned(),
        observed_at_unix_ms,
        agents: CollaborationLiveAgents { agents },
    }
}

fn agent(agent_name: &str, agent_status: serde_json::Value) -> CollaborationLiveAgent {
    CollaborationLiveAgent {
        agent_name: agent_name.to_owned(),
        agent_status,
    }
}

#[tokio::test]
async fn list_agents_snapshots_update_current_rows_and_archive_absent_paths() {
    AgentSessionRegistry::mark_runtime_server_owner_process();
    let state_home = tempfile::tempdir().expect("state home");
    let registry = AgentSessionRegistry::open_or_create_state_root_async(state_home.path())
        .await
        .expect("registry");

    let first = snapshot(
        10,
        vec![
            agent("/root", json!("running")),
            agent("/root/asp_testing", json!({"completed": "green"})),
        ],
    );
    let first_receipt = registry
        .persist_collaboration_snapshot_from_runtime_owner(&first)
        .await
        .expect("persist first snapshot");
    assert_eq!(first_receipt.observed_agent_count, 2);
    assert_eq!(first_receipt.archived_agent_count, 0);

    let second = snapshot(20, vec![agent("/root", json!("running"))]);
    let second_receipt = registry
        .persist_collaboration_snapshot_from_runtime_owner(&second)
        .await
        .expect("persist second snapshot");
    assert_eq!(second_receipt.observed_agent_count, 1);
    assert_eq!(second_receipt.archived_agent_count, 1);

    let current = registry
        .query_collaboration_agents_from_runtime_owner("/workspace/project", "root-session", false)
        .await
        .expect("query current observations");
    assert_eq!(current.len(), 1);
    assert_eq!(current[0].agent_path, "/root");
    assert_eq!(current[0].status_kind, "running");
    assert_eq!(current[0].archived_at_unix_ms, None);

    let all = registry
        .query_collaboration_agents_from_runtime_owner("/workspace/project", "root-session", true)
        .await
        .expect("query all observations");
    assert_eq!(all.len(), 2);
    assert_eq!(all[1].agent_path, "/root/asp_testing");
    assert_eq!(all[1].status_kind, "completed");
    assert_eq!(all[1].archived_at_unix_ms, Some(20));
}

#[tokio::test]
async fn runtime_owner_drains_hook_snapshot_inbox_before_shutdown() {
    AgentSessionRegistry::mark_runtime_server_owner_process();
    let state_home = tempfile::tempdir().expect("state home");
    let registry = Arc::new(
        AgentSessionRegistry::open_or_create_state_root_async(state_home.path())
            .await
            .expect("registry"),
    );
    let inbox = state_home.path().join("agents/live");
    std::fs::create_dir_all(&inbox).expect("inbox");
    let observed = snapshot(30, vec![agent("/root", json!("running"))]);
    std::fs::write(
        inbox.join("snapshot.json"),
        serde_json::to_vec(&observed).expect("encode snapshot"),
    )
    .expect("write snapshot");

    let (shutdown, receiver) = tokio::sync::watch::channel(false);
    let consumer = tokio::spawn(run_collaboration_snapshot_inbox(
        Arc::clone(&registry),
        state_home.path().to_path_buf(),
        receiver,
    ));
    shutdown.send(true).expect("request shutdown");
    consumer
        .await
        .expect("join consumer")
        .expect("consumer terminal");

    assert!(!inbox.join("snapshot.json").exists());
    let rows = registry
        .query_collaboration_agents_from_runtime_owner("/workspace/project", "root-session", false)
        .await
        .expect("query observations");
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].agent_path, "/root");
}
