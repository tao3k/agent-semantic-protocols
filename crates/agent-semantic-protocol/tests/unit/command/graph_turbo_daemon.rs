use serde_json::{Value, json};

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use agent_semantic_client_db::runtime_server::GraphTurboResidentStatusHandle;
use agent_semantic_client_db::runtime_server_control::GraphTurboResidentState;

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
async fn optional_resident_startup_cannot_block_core_readiness() {
    let status = GraphTurboResidentStatusHandle::new(resident_status(
        GraphTurboResidentState::Starting,
        Some("/fixture/graph-turbo".to_owned()),
        "",
    ));
    let observed_status = status.clone();
    let startup: GraphTurboStartup = Box::pin(async {
        tokio::time::sleep(Duration::from_millis(250)).await;
        Err("fixture startup failure".to_owned())
    });

    let construction_started = Instant::now();
    let mut daemon =
        GraphTurboDaemon::start_configured(status, PathBuf::from("/fixture/graph-turbo"), startup);
    assert!(
        construction_started.elapsed() < Duration::from_millis(25),
        "Graph Turbo actor construction blocked core readiness"
    );

    let evaluate = daemon
        .evaluation_builder()
        .expect("configured actor exposes its typed evaluation adapter");
    let rejection_started = Instant::now();
    let error = evaluate(
        "workspace-fixture".to_owned(),
        PathBuf::from("."),
        rank_message("session-fixture"),
    )
    .await
    .expect_err("starting provider must reject without waiting for its handshake");
    assert!(
        error.contains("state=starting"),
        "unexpected error: {error}"
    );
    assert!(
        rejection_started.elapsed() < Duration::from_millis(10),
        "starting provider readiness check exceeded the foreground gate"
    );

    daemon
        .shutdown()
        .await
        .expect("shutdown joins the server-owned startup actor");
    assert_eq!(
        observed_status.snapshot().state,
        GraphTurboResidentState::Unavailable
    );
}
