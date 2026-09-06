use std::os::unix::fs::PermissionsExt as _;
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;
use std::process::Output;
use std::thread;

use agent_semantic_artifacts::runtime_artifact_activation::read_runtime_artifact_activation_event;
use agent_semantic_artifacts::runtime_artifact_publication::publish_runtime_artifact;

const RUNTIME_CLIENT_QUERY_ARGS: &[&str] = &[
    "query",
    "--selector",
    "rust://src/lib.rs#item/function/missing",
    "--workspace",
    ".",
    "--projection",
    "source",
];

fn asp_binary() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_asp"))
}

fn run_asp(state_home: &Path, args: &[&str]) -> Output {
    Command::new(asp_binary())
        .env("ASP_STATE_HOME", state_home)
        .args(args)
        .output()
        .expect("run isolated ASP command")
}

fn run_asp_no_agent(state_home: &Path, args: &[&str]) -> Output {
    Command::new(asp_binary())
        .env("ASP_STATE_HOME", state_home)
        .env("ASP_NO_AGENT", "1")
        .args(args)
        .output()
        .expect("run isolated ASP no-agent command")
}

fn assert_success(output: &Output, operation: &str) {
    assert!(
        output.status.success(),
        "{operation} failed: status={} stdout={} stderr={}",
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

fn assert_workspace_admission_missing(output: &Output, operation: &str) {
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        !output.status.success(),
        "{operation} unexpectedly succeeded: {text}"
    );
    assert!(
        text.contains("Runtime Server workspace admission catalog has no binding"),
        "{operation} did not reach the healthy Runtime workspace boundary: {text}"
    );
}

fn assert_operator_stopped(output: &Output, operation: &str) {
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        !output.status.success(),
        "{operation} unexpectedly succeeded: {text}"
    );
    assert!(
        text.contains("reasonKind=runtime-server-operator-stopped"),
        "{operation} did not preserve operator-stop authority: {text}"
    );
}

async fn publish_pending_runtime(state_home: &Path) {
    let source = asp_binary();
    let target = state_home.join("runtime/bin/asp");
    publish_runtime_artifact(state_home, &source, &target, "dev")
        .await
        .expect("publish immutable Runtime artifact activation");
    assert!(
        read_runtime_artifact_activation_event(state_home)
            .await
            .expect("read Runtime artifact activation")
            .is_some(),
        "publication must leave a durable pending activation event"
    );
}

fn stop_isolated_runtime(state_home: &Path) {
    let output = run_asp(state_home, &["server", "stop"]);
    assert_success(&output, "isolated Runtime stop");
}

#[tokio::test(flavor = "multi_thread")]
async fn pending_activation_bootstrap_waits_for_one_healthy_runtime_owner() {
    let temporary = tempfile::tempdir().expect("create isolated state home");
    let state_home = temporary.path().to_path_buf();
    publish_pending_runtime(&state_home).await;

    let bootstrap = run_asp(&state_home, RUNTIME_CLIENT_QUERY_ARGS);
    assert_workspace_admission_missing(&bootstrap, "pending-activation client bootstrap");
    assert!(
        state_home
            .join("runtime/server/owner-spawn.v1.json")
            .is_file()
    );
    let output = format!(
        "{}{}",
        String::from_utf8_lossy(&bootstrap.stdout),
        String::from_utf8_lossy(&bootstrap.stderr)
    );
    assert!(
        !output.contains("runtime-server-activation-spawn-accepted")
            && !output.contains("runtime-server-activation-owner-starting"),
        "client request returned an intermediate bootstrap observation: {output}"
    );
    assert!(
        !output.contains("runtime-server-activation-failed"),
        "client bootstrap failed before Healthy publication: {output}"
    );
    let transaction =
        agent_semantic_client_db::runtime_server_lifecycle::observe_resident_transaction(
            &state_home,
        )
        .await
        .expect("bootstrap must publish one bound Runtime resident transaction");
    assert_eq!(transaction.state, "ready");

    stop_isolated_runtime(&state_home);
}

#[tokio::test(flavor = "multi_thread")]
async fn explicit_activation_waits_for_one_bound_resident_transaction_without_polling() {
    let temporary = tempfile::tempdir().expect("create isolated state home");
    let state_home = temporary.path().to_path_buf();
    publish_pending_runtime(&state_home).await;

    let start = run_asp(&state_home, &["server", "start"]);
    assert_success(&start, "supervised Runtime activation");
    let transaction =
        agent_semantic_client_db::runtime_server_lifecycle::observe_resident_transaction(
            &state_home,
        )
        .await
        .expect("observe one bound Runtime resident transaction");
    assert_eq!(transaction.state, "ready");
    assert!(!transaction.publication_nonce.is_empty());
    assert_eq!(
        transaction.publication_nonce,
        transaction.applied_publication_nonce
    );
    assert_eq!(
        transaction.launcher_artifact_digest,
        transaction.applied_artifact_digest
    );
    assert_eq!(
        transaction.launcher_artifact_digest,
        transaction.endpoint_binary_content_digest
    );
    assert!(std::path::Path::new(&transaction.launcher_artifact_path).is_absolute());
    assert_eq!(transaction.spawn_argv, ["server", "daemon"]);
    transaction
        .control_endpoint
        .validate()
        .expect("typed loopback control endpoint");
    transaction
        .data_endpoint
        .validate()
        .expect("typed loopback data endpoint");
    transaction
        .provider_endpoint
        .validate()
        .expect("typed loopback provider endpoint");
    assert_eq!(transaction.previous_drain_state, "not-required");

    stop_isolated_runtime(&state_home);
}

#[tokio::test(flavor = "multi_thread")]
async fn restart_replaces_owner_from_applied_generation_in_one_typed_transaction() {
    let temporary = tempfile::tempdir().expect("create isolated state home");
    let state_home = temporary.path().to_path_buf();
    publish_pending_runtime(&state_home).await;

    let start = run_asp(&state_home, &["server", "start"]);
    assert_success(&start, "initial Runtime activation");
    let applied_before = agent_semantic_artifacts::runtime_artifact_activation::
        read_applied_runtime_artifact_activation_event(&state_home)
        .await
        .expect("read initial applied activation")
        .expect("initial applied activation");
    let owner_before =
        agent_semantic_client_db::runtime_server_lifecycle::read_owner_receipt(&state_home)
            .await
            .expect("read initial owner")
            .expect("initial owner");
    let endpoint_before =
        agent_semantic_client_db::runtime_server_control::read_runtime_server_supervisor_endpoint(
            &state_home,
        )
        .await
        .expect("read initial endpoint")
        .expect("initial endpoint");
    assert_eq!(endpoint_before.owner_process_id, owner_before.process_id);

    let restart = run_asp(&state_home, &["server", "restart"]);
    assert_success(&restart, "atomic Runtime restart");
    let receipt: serde_json::Value =
        serde_json::from_slice(&restart.stdout).expect("decode restart receipt");
    assert_eq!(
        receipt["schemaId"],
        "agent.semantic-protocols.runtime-server-restart-receipt.v1"
    );
    assert_eq!(receipt["schemaVersion"], "1");
    assert_eq!(receipt["state"], "healthy");
    assert_eq!(
        receipt["publicationNonce"],
        applied_before.publication_nonce
    );
    assert_eq!(
        receipt["artifactDigest"],
        applied_before.artifact_digest.to_string()
    );

    let owner_after =
        agent_semantic_client_db::runtime_server_lifecycle::read_owner_receipt(&state_home)
            .await
            .expect("read replacement owner")
            .expect("replacement owner");
    let endpoint_after =
        agent_semantic_client_db::runtime_server_control::read_runtime_server_supervisor_endpoint(
            &state_home,
        )
        .await
        .expect("read replacement endpoint")
        .expect("replacement endpoint");
    assert_ne!(endpoint_after.owner_epoch, endpoint_before.owner_epoch);
    assert_eq!(endpoint_after.owner_process_id, owner_after.process_id);
    assert_eq!(receipt["ownerEpoch"], endpoint_after.owner_epoch);
    assert_eq!(
        owner_after.publication_nonce,
        applied_before.publication_nonce
    );
    assert_eq!(
        owner_after.launcher_artifact_digest,
        applied_before.artifact_digest
    );
    assert!(
        read_runtime_artifact_activation_event(&state_home)
            .await
            .expect("read pending after restart")
            .is_none(),
        "restart must not mint or republish a pending publication"
    );

    stop_isolated_runtime(&state_home);
}

#[tokio::test(flavor = "multi_thread")]
async fn no_agent_client_recovers_one_dead_applied_owner_and_respects_operator_stop() {
    let temporary = tempfile::tempdir().expect("create isolated state home");
    let state_home = temporary.path().to_path_buf();
    publish_pending_runtime(&state_home).await;

    let start = run_asp(&state_home, &["server", "start"]);
    assert_success(&start, "initial Runtime activation");
    let applied = agent_semantic_artifacts::runtime_artifact_activation::
        read_applied_runtime_artifact_activation_event(&state_home)
        .await
        .expect("read applied activation")
        .expect("applied activation");
    let owner_before =
        agent_semantic_client_db::runtime_server_lifecycle::read_owner_receipt(&state_home)
            .await
            .expect("read initial owner")
            .expect("initial owner");
    let endpoint_before =
        agent_semantic_client_db::runtime_server_control::read_runtime_server_supervisor_endpoint(
            &state_home,
        )
        .await
        .expect("read initial endpoint")
        .expect("initial endpoint");
    assert_eq!(endpoint_before.owner_process_id, owner_before.process_id);

    agent_semantic_runtime::runtime_process_lifecycle::force_terminate(owner_before.process_id)
        .await
        .expect("terminate isolated Runtime owner");
    let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(5);
    while agent_semantic_runtime::runtime_process_lifecycle::process_id_is_alive(
        owner_before.process_id,
    )
    .await
    {
        assert!(
            tokio::time::Instant::now() < deadline,
            "isolated Runtime owner did not exit"
        );
        tokio::time::sleep(std::time::Duration::from_millis(5)).await;
    }

    let recovered = run_asp_no_agent(&state_home, RUNTIME_CLIENT_QUERY_ARGS);
    assert_workspace_admission_missing(&recovered, "dead-owner recovery query");
    let recovery_output = format!(
        "{}{}",
        String::from_utf8_lossy(&recovered.stdout),
        String::from_utf8_lossy(&recovered.stderr)
    );
    assert!(
        !recovery_output.contains("runtime-server-activation-spawn-accepted")
            && !recovery_output.contains("runtime-server-owner-stale")
            && !recovery_output.contains("Connection refused"),
        "no-agent client returned before recovery completed: {recovery_output}"
    );
    let owner_after =
        agent_semantic_client_db::runtime_server_lifecycle::read_owner_receipt(&state_home)
            .await
            .expect("read recovered owner")
            .expect("recovered owner");
    let endpoint_after =
        agent_semantic_client_db::runtime_server_control::read_runtime_server_supervisor_endpoint(
            &state_home,
        )
        .await
        .expect("read recovered endpoint")
        .expect("recovered endpoint");
    assert!(
        agent_semantic_runtime::runtime_process_lifecycle::process_id_is_alive(
            endpoint_after.owner_process_id
        )
        .await,
        "no-agent command left a dead endpoint authority: {recovery_output}"
    );
    assert_ne!(endpoint_after.owner_epoch, endpoint_before.owner_epoch);
    assert_eq!(endpoint_after.owner_process_id, owner_after.process_id);
    assert_eq!(owner_after.publication_nonce, applied.publication_nonce);
    assert_eq!(
        owner_after.launcher_artifact_digest,
        applied.artifact_digest
    );
    let transaction =
        agent_semantic_client_db::runtime_server_lifecycle::observe_resident_transaction(
            &state_home,
        )
        .await
        .expect("observe recovered resident transaction");
    assert_eq!(transaction.state, "ready");
    assert_eq!(transaction.publication_nonce, applied.publication_nonce);
    assert_eq!(
        transaction.launcher_artifact_digest,
        applied.artifact_digest
    );

    stop_isolated_runtime(&state_home);
    let stopped = run_asp_no_agent(&state_home, RUNTIME_CLIENT_QUERY_ARGS);
    assert!(
        !stopped.status.success(),
        "operator-stop must remain authoritative in no-agent mode"
    );
    let stopped_output = format!(
        "{}{}",
        String::from_utf8_lossy(&stopped.stdout),
        String::from_utf8_lossy(&stopped.stderr)
    );
    assert!(
        stopped_output.contains("runtime-server-operator-stopped"),
        "missing typed operator-stop denial: {stopped_output}"
    );
    assert!(
        agent_semantic_client_db::runtime_server_lifecycle::read_owner_receipt(&state_home)
            .await
            .expect("read owner after stopped no-agent request")
            .is_none(),
        "no-agent recovery must not cross operator-stop authority"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn activation_child_exit_terminalizes_and_preserves_pending_without_polling() {
    let temporary = tempfile::tempdir().expect("create isolated state home");
    let state_home = temporary.path().to_path_buf();
    let source = temporary.path().join("exiting-asp");
    std::fs::write(&source, b"#!/bin/sh\nexit 17\n").expect("write exiting Runtime candidate");
    std::fs::set_permissions(&source, std::fs::Permissions::from_mode(0o755))
        .expect("make exiting Runtime candidate executable");
    publish_runtime_artifact(
        &state_home,
        &source,
        &state_home.join("runtime/bin/asp"),
        "dev",
    )
    .await
    .expect("publish exiting Runtime candidate");

    let start = run_asp(&state_home, &["server", "start"]);
    assert!(
        !start.status.success(),
        "exiting Runtime candidate must fail"
    );
    let terminal = String::from_utf8_lossy(&start.stderr);
    assert!(
        terminal.contains("runtime-owner-exited-before-healthy"),
        "unexpected child-exit terminal: {terminal}"
    );
    assert!(
        read_runtime_artifact_activation_event(&state_home)
            .await
            .expect("read activation after child exit")
            .is_some(),
        "child exit before ready must preserve the pending activation"
    );
    assert!(
        agent_semantic_artifacts::runtime_artifact_activation::
            read_applied_runtime_artifact_activation_event(&state_home)
            .await
            .expect("read applied activation after child exit")
            .is_none(),
        "child exit before ready must not forge an applied activation"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn concurrent_pending_activation_bootstraps_share_one_runtime_server_owner() {
    let temporary = tempfile::tempdir().expect("create isolated state home");
    let state_home = temporary.path().to_path_buf();
    publish_pending_runtime(&state_home).await;

    let first_home = state_home.clone();
    let second_home = state_home.clone();
    let first = thread::spawn(move || run_asp(&first_home, RUNTIME_CLIENT_QUERY_ARGS));
    let second = thread::spawn(move || run_asp(&second_home, RUNTIME_CLIENT_QUERY_ARGS));
    assert_workspace_admission_missing(
        &first.join().expect("join first bootstrap"),
        "first bootstrap",
    );
    assert_workspace_admission_missing(
        &second.join().expect("join second bootstrap"),
        "second bootstrap",
    );

    let owner_receipt = std::fs::read(state_home.join("runtime/server/owner-spawn.v1.json"))
        .expect("read canonical owner receipt");
    let owner: serde_json::Value =
        serde_json::from_slice(&owner_receipt).expect("decode canonical owner receipt");
    assert!(
        owner
            .get("processId")
            .and_then(serde_json::Value::as_u64)
            .is_some()
    );

    stop_isolated_runtime(&state_home);
}

#[tokio::test(flavor = "multi_thread")]
async fn operator_stop_suppresses_old_activation_until_a_newer_publication() {
    let temporary = tempfile::tempdir().expect("create isolated state home");
    let state_home = temporary.path().to_path_buf();
    publish_pending_runtime(&state_home).await;
    let stopped_event = read_runtime_artifact_activation_event(&state_home)
        .await
        .expect("read stopped-through activation")
        .expect("pending activation");
    agent_semantic_client_db::runtime_server_lifecycle::mark_operator_stopped(&state_home)
        .await
        .expect("mark operator stopped");

    let suppressed = run_asp(&state_home, RUNTIME_CLIENT_QUERY_ARGS);
    assert_operator_stopped(&suppressed, "suppressed client bootstrap");
    assert!(
        !state_home
            .join("runtime/server/owner-spawn.v1.json")
            .exists(),
        "healthcheck/bootstrap must not cross operator-stop authority"
    );
    let tombstone =
        agent_semantic_client_db::runtime_server_lifecycle::read_operator_stop_receipt(&state_home)
            .await
            .expect("read operator-stop receipt")
            .expect("operator-stop receipt");
    assert_eq!(
        tombstone.stopped_artifact_digest,
        Some(stopped_event.artifact_digest.clone())
    );
    assert_eq!(
        tombstone.stopped_publication_nonce,
        Some(stopped_event.publication_nonce.clone())
    );

    publish_pending_runtime(&state_home).await;
    let newer_event = read_runtime_artifact_activation_event(&state_home)
        .await
        .expect("read newer activation")
        .expect("newer pending activation");
    assert_ne!(
        newer_event.publication_nonce,
        stopped_event.publication_nonce
    );

    let resumed = run_asp(&state_home, RUNTIME_CLIENT_QUERY_ARGS);
    assert_workspace_admission_missing(&resumed, "newer activation bootstrap");
    assert!(
        state_home
            .join("runtime/server/owner-spawn.v1.json")
            .is_file(),
        "distinct content-bound publication must reacquire supervisor authority"
    );

    stop_isolated_runtime(&state_home);
}

#[tokio::test(flavor = "multi_thread")]
async fn current_schema_v1_operator_stop_suppresses_covered_activation() {
    let temporary = tempfile::tempdir().expect("create isolated state home");
    let state_home = temporary.path().to_path_buf();
    publish_pending_runtime(&state_home).await;
    let stopped_event = read_runtime_artifact_activation_event(&state_home)
        .await
        .expect("read stopped-through activation")
        .expect("pending activation");
    let stop_path = state_home.join("runtime/server/operator-stop.v1.json");
    std::fs::create_dir_all(stop_path.parent().expect("operator stop parent"))
        .expect("create operator stop parent");
    std::fs::write(
        &stop_path,
        serde_json::to_vec(&serde_json::json!({
            "schemaId": "agent.semantic-protocols.runtime-server-operator-stop",
            "schemaVersion": "1",
            "state": "stopped",
            "stoppedArtifactDigest": stopped_event.artifact_digest.clone(),
            "stoppedPublicationNonce": stopped_event.publication_nonce.clone(),
        }))
        .expect("encode current schema v1 operator stop"),
    )
    .expect("seed current schema v1 operator stop");

    let suppressed = run_asp(&state_home, RUNTIME_CLIENT_QUERY_ARGS);
    assert_operator_stopped(&suppressed, "current v1 suppressed bootstrap");
    assert!(
        !state_home
            .join("runtime/server/owner-spawn.v1.json")
            .exists()
    );
    let canonical =
        agent_semantic_client_db::runtime_server_lifecycle::read_operator_stop_receipt(&state_home)
            .await
            .expect("read current operator stop")
            .expect("current operator stop");
    assert_eq!(
        canonical.stopped_artifact_digest,
        Some(stopped_event.artifact_digest.clone())
    );
    assert_eq!(
        canonical.stopped_publication_nonce,
        Some(stopped_event.publication_nonce.clone())
    );

    publish_pending_runtime(&state_home).await;
    let resumed = run_asp(&state_home, RUNTIME_CLIENT_QUERY_ARGS);
    assert_workspace_admission_missing(&resumed, "newer activation after canonical stop");
    assert!(
        state_home
            .join("runtime/server/owner-spawn.v1.json")
            .is_file()
    );
    stop_isolated_runtime(&state_home);
}

#[tokio::test(flavor = "multi_thread")]
async fn invalid_operator_stop_receipts_fail_closed_without_starting_an_owner() {
    let cases = [
        ("malformed", "{"),
        (
            "stale-v1-missing-generation",
            r#"{"schemaId":"agent.semantic-protocols.runtime-server-operator-stop","schemaVersion":"1","state":"stopped"}"#,
        ),
        (
            "unreleased-schema-version-2",
            r#"{"schemaId":"agent.semantic-protocols.runtime-server-operator-stop","schemaVersion":"2","state":"stopped"}"#,
        ),
        (
            "unknown-version",
            r#"{"schemaId":"agent.semantic-protocols.runtime-server-operator-stop","schemaVersion":"9","state":"stopped","stoppedThroughActivationGeneration":0}"#,
        ),
    ];
    for (case, receipt) in cases {
        let temporary = tempfile::tempdir().expect("create isolated state home");
        let state_home = temporary.path().to_path_buf();
        publish_pending_runtime(&state_home).await;
        let stop_path = state_home.join("runtime/server/operator-stop.v1.json");
        std::fs::create_dir_all(stop_path.parent().expect("operator stop parent"))
            .expect("create operator stop parent");
        std::fs::write(&stop_path, receipt).expect("seed invalid operator stop");

        let bootstrap = run_asp(&state_home, RUNTIME_CLIENT_QUERY_ARGS);
        assert!(
            !bootstrap.status.success(),
            "{case} operator-stop receipt must fail closed"
        );
        let failure = format!(
            "{}{}",
            String::from_utf8_lossy(&bootstrap.stdout),
            String::from_utf8_lossy(&bootstrap.stderr)
        );
        assert!(failure.contains("operator-stop"), "{case}: {failure}");
        assert!(stop_path.is_file(), "{case} marker must be preserved");
        assert!(
            !state_home
                .join("runtime/server/owner-spawn.v1.json")
                .exists(),
            "{case} marker must not start an owner"
        );
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn concurrent_current_schema_v1_reads_are_idempotent() {
    let temporary = tempfile::tempdir().expect("create isolated state home");
    let state_home = temporary.path().to_path_buf();
    publish_pending_runtime(&state_home).await;
    let stopped_event = read_runtime_artifact_activation_event(&state_home)
        .await
        .expect("read stopped-through activation")
        .expect("pending activation");
    let stop_path = state_home.join("runtime/server/operator-stop.v1.json");
    std::fs::create_dir_all(stop_path.parent().expect("operator stop parent"))
        .expect("create operator stop parent");
    std::fs::write(
        &stop_path,
        serde_json::to_vec(&serde_json::json!({
            "schemaId": "agent.semantic-protocols.runtime-server-operator-stop",
            "schemaVersion": "1",
            "state": "stopped",
            "stoppedArtifactDigest": stopped_event.artifact_digest.clone(),
            "stoppedPublicationNonce": stopped_event.publication_nonce.clone(),
        }))
        .expect("encode current schema v1 operator stop"),
    )
    .expect("seed current schema v1 operator stop");

    let (first, second) = tokio::join!(
        agent_semantic_client_db::runtime_server_lifecycle::read_operator_stop_receipt(&state_home),
        agent_semantic_client_db::runtime_server_lifecycle::read_operator_stop_receipt(&state_home)
    );
    let first = first
        .expect("first current v1 read")
        .expect("first receipt");
    let second = second
        .expect("second current v1 read")
        .expect("second receipt");
    assert_eq!(first, second);

    let current_receipt =
        agent_semantic_client_db::runtime_server_lifecycle::read_operator_stop_receipt(&state_home)
            .await
            .expect("read current receipt")
            .expect("current receipt");
    assert_eq!(current_receipt.schema_version, "1");
    assert_eq!(
        current_receipt.stopped_artifact_digest,
        Some(stopped_event.artifact_digest.clone())
    );
    assert_eq!(
        current_receipt.stopped_publication_nonce,
        Some(stopped_event.publication_nonce.clone())
    );
    assert!(stop_path.is_file());
    assert!(
        !state_home
            .join("runtime/server/owner-spawn.v1.json")
            .exists()
    );
}
