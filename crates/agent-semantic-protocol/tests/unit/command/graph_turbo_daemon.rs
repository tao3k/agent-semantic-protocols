use serde_json::{Value, json};

use std::path::{Path, PathBuf};

use crate::server::runtime_server_supervisor::{
    ConfiguredGraphTurboArtifact, configured_graph_turbo_artifact_at_state_home,
};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use agent_semantic_client_db::runtime_server::GraphTurboResidentStatusHandle;
use agent_semantic_client_db::runtime_server_control::GraphTurboResidentState;
use tokio::sync::Barrier;

use super::{
    GraphTurboDaemon, GraphTurboStartupFactory, admit_exact_candidate, command_digest,
    resident_status, start_resident, validate_rank_request,
};

fn rank_message(workspace_identity: &str) -> Value {
    json!({
        "schemaId": "agent.semantic-protocols.graph-turbo-resident-server",
        "schemaVersion": "1",
        "messageKind": "rank",
        "requestId": 7,
        "workspaceIdentity": workspace_identity,
        "generationDigest": "blake3-256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        "pageRoots": {"owners": "blake3-256:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"},
        "terms": ["runtime"],
        "rankPayload": {"graph": {"nodes": [], "edges": []}}
    })
}

fn rank_receipt(workspace_identity: &str) -> Value {
    json!({
        "schemaId": "agent.semantic-protocols.graph-turbo-resident-server",
        "schemaVersion": "1",
        "messageKind": "receipt",
        "requestId": 7,
        "workspaceIdentity": workspace_identity,
        "generationDigest": "blake3-256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        "state": "completed",
        "rankedNodes": [],
        "processSpawns": 0,
        "generationLoads": 0,
        "graphPageReads": 1,
        "result": {"schemaId": "agent.semantic-protocols.semantic-graph-turbo-result", "schemaVersion": "1"}
    })
}

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("workspace root")
        .to_path_buf()
}

fn rank_message_with_id(workspace_identity: &str, request_id: u64) -> Value {
    let mut message = rank_message(workspace_identity);
    message["requestId"] = Value::from(request_id);
    message
}

#[test]
fn exact_candidate_echo_is_admitted() {
    assert!(
        admit_exact_candidate(&rank_message("workspace-a"), rank_receipt("workspace-a")).is_ok()
    );
}

#[test]
fn cross_session_or_authority_substitution_is_rejected() {
    let request = rank_message("workspace-a");
    assert!(admit_exact_candidate(&request, rank_receipt("workspace-b")).is_err());
    let mut stale = rank_receipt("workspace-a");
    stale["generationDigest"] = Value::String(
        "blake3-256:cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc".to_owned(),
    );
    assert!(admit_exact_candidate(&request, stale).is_err());
}

#[test]
fn daemon_rejects_unknown_resident_version() {
    let mut request = rank_message("session-a");
    request["schemaVersion"] = Value::String("999".to_owned());
    assert!(validate_rank_request(&request).is_err());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn failed_supervisor_bootstrap_is_not_retried_by_queries() {
    let status = GraphTurboResidentStatusHandle::new(resident_status(
        GraphTurboResidentState::Unavailable,
        Some("/fixture/graph-turbo".to_owned()),
        "graph-turbo-supervisor-pending",
    ));
    let observed_status = status.clone();
    let startup_attempts = Arc::new(AtomicUsize::new(0));
    let observed_attempts = startup_attempts.clone();
    let startup: GraphTurboStartupFactory = Arc::new(move || {
        let observed_attempts = observed_attempts.clone();
        Box::pin(async move {
            observed_attempts.fetch_add(1, Ordering::Relaxed);
            Err("fixture startup failure".to_owned())
        })
    });
    let mut daemon = GraphTurboDaemon::start_configured(
        status,
        Some(PathBuf::from("/fixture/graph-turbo")),
        startup,
    );
    let evaluate = daemon.evaluation_builder().expect("evaluation adapter");

    tokio::time::timeout(Duration::from_millis(100), async {
        while startup_attempts.load(Ordering::Relaxed) < 1
            || observed_status.snapshot().state != GraphTurboResidentState::Failed
        {
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("failed supervisor bootstrap terminal receipt");

    for request_id in 1..=2 {
        let error = evaluate(
            "workspace-a".to_owned(),
            PathBuf::from("."),
            rank_message(&format!("session-{request_id}")),
        )
        .await
        .expect_err("queries cannot restart a failed supervisor-owned resident");
        assert!(error.contains("state=failed"));
    }

    assert_eq!(startup_attempts.load(Ordering::Relaxed), 1);
    daemon.shutdown().await.expect("daemon shutdown");
}

#[test]
fn execution_command_digest_binds_the_configured_locator() {
    assert_ne!(
        command_digest(Path::new("/venv/bin/python")),
        command_digest(Path::new("/base/bin/python"))
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn runtime_supervisor_bootstraps_one_process_for_concurrent_workspaces_and_drains() {
    let root = workspace_root();
    let artifact = root.join("packages/python/.venv/bin/asp-graph-turbo-resident");
    let runtime_artifact_digest = format!(
        "blake3-256:{}",
        crate::command::protocol_binary::canonical_protocol_binary_artifact_digest(&artifact)
            .await
            .expect("digest standalone Graph Turbo resident artifact")
    );
    let configured = ConfiguredGraphTurboArtifact {
        locator: artifact.clone(),
        runtime_artifact_digest,
    };
    let startup_artifact = configured.clone();
    let startup_root = root.clone();
    let startup: GraphTurboStartupFactory = Arc::new(move || {
        let configured = startup_artifact.clone();
        let root = startup_root.clone();
        Box::pin(async move { start_resident(configured, &root).await })
    });
    let status = GraphTurboResidentStatusHandle::new(resident_status(
        GraphTurboResidentState::Unavailable,
        Some(artifact.display().to_string()),
        "fixture-supervisor-pending",
    ));
    let observed_status = status.clone();
    let mut daemon = GraphTurboDaemon::start_configured(status, Some(artifact), startup);
    let evaluate = daemon.evaluation_builder().expect("evaluation adapter");
    let healthy = tokio::time::timeout(Duration::from_secs(5), async {
        while observed_status.snapshot().state != GraphTurboResidentState::Healthy {
            tokio::task::yield_now().await;
        }
    })
    .await;
    assert!(
        healthy.is_ok(),
        "managed Graph Turbo resident becomes healthy: status={:?}",
        observed_status.snapshot()
    );

    let mut requests = tokio::task::JoinSet::new();
    for workspace in 0..8_u64 {
        for session in 0..32_u64 {
            let evaluate = evaluate.clone();
            let root = root.clone();
            requests.spawn(async move {
                let request_id = 100 + workspace * 32 + session;
                evaluate(
                    format!("workspace-{workspace}"),
                    root,
                    rank_message_with_id(&format!("workspace-{workspace}"), request_id),
                )
                .await
            });
        }
    }
    while let Some(result) = requests.join_next().await {
        result
            .expect("join concurrent Graph Turbo request")
            .expect("every concurrent workspace request receives a v1 receipt");
    }
    daemon
        .shutdown()
        .await
        .expect("drain joins the one managed child");
    assert_eq!(
        observed_status.snapshot().state,
        GraphTurboResidentState::Unavailable,
        "Runtime drain must leave no resident Graph Turbo actor"
    );
}

#[tokio::test]
async fn resident_config_rejects_every_non_v1_protocol_identity() {
    let state_home = tempfile::tempdir().expect("state home");
    let server_dir = state_home.path().join("runtime/server");
    std::fs::create_dir_all(&server_dir).expect("server dir");
    std::fs::write(
        server_dir.join("graph-turbo-resident-config.v1.json"),
        br#"{
          "schemaId":"agent.semantic-protocols.semantic-graph-turbo-resident-config",
          "schemaVersion":"2",
          "artifactKind":"standalone-directory",
          "executionArtifactLocator":"/invalid",
          "executionArtifactDigest":"blake3-256:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
          "runtimeArtifactDigest":"blake3-256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
        }"#,
    )
    .expect("config");

    let error = configured_graph_turbo_artifact_at_state_home(state_home.path())
        .await
        .expect_err("non-v1 resident config must fail closed");
    assert_eq!(error, "Graph Turbo resident config protocol identity drift");
}

#[tokio::test]
async fn resident_config_binds_managed_locator_to_its_content_digest() {
    let state_home = tempfile::tempdir().expect("state home");
    let release = state_home.path().join("release");
    let bundle = release.join("asp-graph-turbo-resident.bundle");
    tokio::fs::create_dir_all(bundle.join("_internal"))
        .await
        .expect("bundle root");
    let asp = release.join(format!("asp{}", std::env::consts::EXE_SUFFIX));
    tokio::fs::write(&asp, b"asp").await.expect("ASP candidate");
    tokio::fs::write(
        bundle.join(format!(
            "asp-graph-turbo-resident{}",
            std::env::consts::EXE_SUFFIX
        )),
        b"managed graph turbo resident",
    )
    .await
    .expect("resident entry");
    tokio::fs::write(bundle.join("_internal/runtime.dat"), b"embedded runtime")
        .await
        .expect("embedded runtime");
    let published =
        crate::runtime_artifact::publish_graph_turbo_resident_sibling(state_home.path(), &asp)
            .await
            .expect("publish standalone bundle");

    let configured = configured_graph_turbo_artifact_at_state_home(state_home.path())
        .await
        .expect("read typed config")
        .expect("configured artifact");
    assert_eq!(
        configured.runtime_artifact_digest,
        published.runtime_artifact_digest
    );
    assert_eq!(
        configured.locator,
        published.locator.canonicalize().expect("canonical entry")
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn supervisor_bootstrap_rejects_config_without_repairing_it() {
    let state_home = tempfile::tempdir().unwrap();
    let config_dir = state_home.path().join("runtime").join("server");
    std::fs::create_dir_all(&config_dir).unwrap();
    let config = config_dir.join("graph-turbo-resident-config.v1.json");
    std::fs::write(&config, b"rejected-config-must-remain-untouched").unwrap();

    let mut daemon = GraphTurboDaemon::start_from_managed_config(state_home.path()).await;
    tokio::time::timeout(Duration::from_millis(100), async {
        while daemon.status().snapshot().state != GraphTurboResidentState::Failed {
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("supervisor publishes rejected config as a terminal receipt");
    let failed = daemon.status().snapshot();
    assert!(
        failed
            .reason
            .as_deref()
            .is_some_and(|reason| reason.contains("decode Graph Turbo resident config")),
        "unexpected supervisor failure: {failed:?}"
    );
    assert_eq!(
        std::fs::read(&config).unwrap(),
        b"rejected-config-must-remain-untouched"
    );

    daemon.shutdown().await.unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn supervisor_bootstrap_is_non_blocking_and_queries_never_poll_startup() {
    let status = GraphTurboResidentStatusHandle::new(resident_status(
        GraphTurboResidentState::Unavailable,
        Some("/fixture/graph-turbo".to_owned()),
        "graph-turbo-supervisor-pending",
    ));
    let observed_status = status.clone();
    let startup_polls = Arc::new(AtomicUsize::new(0));
    let observed_startup_polls = Arc::clone(&startup_polls);
    let startup_release = Arc::new(tokio::sync::Notify::new());
    let actor_startup_release = startup_release.clone();
    let startup: GraphTurboStartupFactory = Arc::new(move || {
        let observed_startup_polls = observed_startup_polls.clone();
        let startup_release = actor_startup_release.clone();
        Box::pin(async move {
            observed_startup_polls.fetch_add(1, Ordering::Relaxed);
            startup_release.notified().await;
            Err("fixture startup failure".to_owned())
        })
    });

    let mut daemon = GraphTurboDaemon::start_configured(
        status,
        Some(PathBuf::from("/fixture/graph-turbo")),
        startup,
    );
    tokio::time::timeout(Duration::from_millis(100), async {
        while startup_polls.load(Ordering::Relaxed) != 1 {
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("supervisor polls the Graph startup future once");

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
                .expect_err("query must return before supervisor startup completes");
                (started.elapsed(), error)
            });
        }
    }
    barrier.wait().await;
    let mut elapsed_samples = Vec::with_capacity(participant_count);
    while let Some(result) = tasks.join_next().await {
        let (elapsed, error) = result.unwrap();
        elapsed_samples.push(elapsed);
        assert!(
            error.contains("state=starting"),
            "unexpected error: {error}"
        );
    }
    elapsed_samples.sort_unstable();
    let p99_index = (elapsed_samples.len() * 99).div_ceil(100) - 1;
    let p99 = elapsed_samples[p99_index];
    let max = *elapsed_samples.last().expect("lease-miss latency sample");
    assert!(
        p99 < Duration::from_millis(1),
        "Graph Turbo lease-miss p99 exceeded the foreground gate: p99={p99:?} max={max:?}"
    );
    assert_eq!(
        startup_polls.load(Ordering::Relaxed),
        1,
        "queries must not poll or restart the supervisor-owned startup future"
    );
    startup_release.notify_one();

    tokio::time::timeout(Duration::from_millis(100), async {
        while observed_status.snapshot().state != GraphTurboResidentState::Failed {
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("supervisor startup failure must publish a bounded terminal receipt");
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
