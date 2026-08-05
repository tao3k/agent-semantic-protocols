use serde_json::{Value, json};

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use agent_semantic_client_db::runtime_server::GraphTurboResidentStatusHandle;
use agent_semantic_client_db::runtime_server_control::GraphTurboResidentState;
use tokio::sync::Barrier;

use super::{
    GraphTurboDaemon, GraphTurboStartup, admit_exact_candidate, command_digest, resident_status,
    validate_rank_request,
};

fn rank_message(session_id: &str) -> Value {
    json!({
        "schemaVersion": "1",
        "messageKind": "rank",
        "sessionId": session_id,
        "nodeId": "router",
        "snapshotDigest": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        "workspaceGenerationRootDigest": "generation",
        "routeId": "route"
    })
}

fn rank_receipt(session_id: &str, authority: &str) -> Value {
    json!({
        "schemaVersion": "1",
        "status": "rank-completed",
        "authority": authority,
        "graphSessionIdentity": {
            "sessionId": session_id,
            "nodeId": "router",
            "snapshotDigest": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            "workspaceGenerationRootDigest": "generation",
            "routeId": "route"
        }
    })
}

#[test]
fn exact_candidate_echo_is_admitted() {
    assert!(
        admit_exact_candidate(
            &rank_message("session-a"),
            rank_receipt("session-a", "candidate")
        )
        .is_ok()
    );
}

#[test]
fn cross_session_or_authority_substitution_is_rejected() {
    let request = rank_message("session-a");
    assert!(admit_exact_candidate(&request, rank_receipt("session-b", "candidate")).is_err());
    assert!(admit_exact_candidate(&request, rank_receipt("session-a", "proved")).is_err());
}

#[test]
fn daemon_rejects_unknown_resident_version() {
    let mut request = rank_message("session-a");
    request["schemaVersion"] = Value::String("999".to_owned());
    assert!(validate_rank_request(&request).is_err());
}

#[test]
fn execution_command_digest_binds_the_configured_locator() {
    assert_ne!(
        command_digest(Path::new("/venv/bin/python")),
        command_digest(Path::new("/base/bin/python"))
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn idle_daemon_does_not_read_or_repair_rejected_graph_config() {
    let state_home = tempfile::tempdir().unwrap();
    let config_dir = state_home.path().join("runtime").join("server");
    std::fs::create_dir_all(&config_dir).unwrap();
    let config = config_dir.join("graph-turbo-resident-config.v1.json");
    std::fs::write(&config, b"rejected-config-must-remain-untouched").unwrap();

    let construction_started = Instant::now();
    let mut daemon = GraphTurboDaemon::start_from_environment(state_home.path()).await;
    assert!(
        construction_started.elapsed() < Duration::from_millis(25),
        "idle Graph Turbo construction performed config I/O"
    );
    tokio::task::yield_now().await;
    assert_eq!(
        daemon.status().snapshot().state,
        GraphTurboResidentState::Unavailable
    );
    assert_eq!(
        std::fs::read(&config).unwrap(),
        b"rejected-config-must-remain-untouched"
    );

    daemon.shutdown().await.unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn optional_resident_is_lazy_and_cannot_block_core_readiness() {
    let status = GraphTurboResidentStatusHandle::new(resident_status(
        GraphTurboResidentState::Unavailable,
        Some("/fixture/graph-turbo".to_owned()),
        "graph-turbo-lazy-not-started",
    ));
    let observed_status = status.clone();
    let startup_polls = Arc::new(AtomicUsize::new(0));
    let observed_startup_polls = Arc::clone(&startup_polls);
    let startup: GraphTurboStartup = Box::pin(async move {
        observed_startup_polls.fetch_add(1, Ordering::Relaxed);
        tokio::time::sleep(Duration::from_millis(250)).await;
        Err("fixture startup failure".to_owned())
    });

    let construction_started = Instant::now();
    let mut daemon = GraphTurboDaemon::start_configured(
        status,
        Some(PathBuf::from("/fixture/graph-turbo")),
        startup,
    );
    assert!(
        construction_started.elapsed() < Duration::from_millis(25),
        "Graph Turbo actor construction blocked core readiness"
    );
    tokio::task::yield_now().await;
    assert_eq!(
        startup_polls.load(Ordering::Relaxed),
        0,
        "idle Runtime Server must not poll the optional Graph startup future"
    );

    let evaluate = daemon
        .evaluation_builder()
        .expect("configured actor exposes its typed evaluation adapter");
    const WORKSPACES: usize = 8;
    const SESSIONS_PER_WORKSPACE: usize = 32;
    let participant_count = WORKSPACES * SESSIONS_PER_WORKSPACE;
    let barrier = Arc::new(Barrier::new(participant_count + 1));
    let mut tasks = tokio::task::JoinSet::new();
    for workspace in 0..WORKSPACES {
        for session in 0..SESSIONS_PER_WORKSPACE {
            let evaluate = evaluate.clone();
            let barrier = barrier.clone();
            tasks.spawn(async move {
                barrier.wait().await;
                let started = Instant::now();
                let error = evaluate(
                    format!("workspace-{workspace}"),
                    PathBuf::from("."),
                    rank_message(&format!("session-{workspace}-{session}")),
                )
                .await
                .expect_err("warm demand must return before resident startup");
                (started.elapsed(), error)
            });
        }
    }
    barrier.wait().await;
    let mut warm_publications = 0;
    let mut starting_followers = 0;
    while let Some(result) = tasks.join_next().await {
        let (elapsed, error) = result.unwrap();
        assert!(
            elapsed < Duration::from_millis(10),
            "Graph Turbo warm admission exceeded the foreground gate: {elapsed:?}"
        );
        if error.contains("warm admission scheduled") {
            warm_publications += 1;
        } else {
            assert!(
                error.contains("state=starting"),
                "unexpected error: {error}"
            );
            starting_followers += 1;
        }
    }
    assert_eq!(warm_publications, 1);
    assert_eq!(starting_followers, participant_count - 1);

    tokio::time::timeout(Duration::from_secs(1), async {
        while observed_status.snapshot().state != GraphTurboResidentState::Failed {
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("lazy startup failure must publish a bounded terminal receipt");
    assert_eq!(startup_polls.load(Ordering::Relaxed), 1);

    daemon
        .shutdown()
        .await
        .expect("shutdown joins the server-owned startup actor");
    assert_eq!(
        observed_status.snapshot().state,
        GraphTurboResidentState::Unavailable
    );
}
