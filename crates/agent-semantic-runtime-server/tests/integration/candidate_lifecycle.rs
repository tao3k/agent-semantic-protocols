// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Candidate Runtime lifecycle integration tests.

use agent_semantic_artifacts::runtime_artifact_retention::RuntimeArtifactMutationGuard;
use agent_semantic_runtime::runtime_process_lifecycle::RuntimeProcessLaunchSpec;
use agent_semantic_runtime::runtime_process_lifecycle::launch_monitored;
use agent_semantic_runtime_server::readiness::RuntimeServerReadinessListener;
use agent_semantic_runtime_server::readiness::RuntimeServerReadinessReceipt;
use agent_semantic_runtime_server::readiness::RuntimeServerReadinessState;
use agent_semantic_runtime_server::readiness::await_monitored_candidate_readiness;
use agent_semantic_runtime_server::readiness::launch_candidate_and_await_readiness;
use agent_semantic_runtime_server::resident_publication::resident_readiness_root;
use std::path::Path;

async fn listener(
    state_home: &Path,
    request_id: &str,
    token: &str,
) -> RuntimeServerReadinessListener {
    let root = resident_readiness_root(state_home)
        .await
        .expect("derive candidate readiness root");
    RuntimeServerReadinessListener::bind_root(&root, request_id, token)
        .await
        .expect("bind candidate readiness endpoint")
}

fn launch_spec(program: &Path, args: Vec<String>, stderr: &Path) -> RuntimeProcessLaunchSpec {
    RuntimeProcessLaunchSpec {
        program: program.to_path_buf(),
        args,
        current_dir: None,
        environment: Vec::new(),
        stderr: stderr.to_path_buf(),
    }
}

#[tokio::test]
async fn spawn_failure_is_typed_immediate_and_releases_artifact_mutation_lock() {
    let temporary = tempfile::Builder::new()
        .prefix("asp-r-")
        .tempdir_in("/tmp")
        .expect("short candidate lifecycle fixture");
    let state_home = temporary.path().join("state");
    let artifact_root = state_home.join("runtime").join("artifacts");
    tokio::fs::create_dir_all(&artifact_root)
        .await
        .expect("create artifact root");
    let listener = listener(&state_home, "spawn-failure", "no-child").await;
    {
        let _guard = RuntimeArtifactMutationGuard::try_acquire(&artifact_root)
            .expect("acquire install lock");
        let error = match launch_candidate_and_await_readiness(
            &listener,
            launch_spec(
                &temporary.path().join("missing-candidate"),
                Vec::new(),
                &temporary.path().join("missing.stderr"),
            ),
        )
        .await
        {
            Ok(_) => panic!("missing candidate must fail before receive"),
            Err(error) => error,
        };
        assert!(error.contains("reasonKind=runtime-server-candidate-spawn-failed"));
    }
    let _reacquired = RuntimeArtifactMutationGuard::try_acquire(&artifact_root)
        .expect("spawn failure must release install lock");
}

#[tokio::test]
async fn child_exit_before_readiness_is_a_typed_terminal() {
    let temporary = tempfile::Builder::new()
        .prefix("asp-r-")
        .tempdir_in("/tmp")
        .expect("short candidate lifecycle fixture");
    let state_home = temporary.path().join("state");
    tokio::fs::create_dir_all(&state_home)
        .await
        .expect("create state home");
    let listener = listener(&state_home, "child-exit", "candidate").await;
    let error = match launch_candidate_and_await_readiness(
        &listener,
        launch_spec(
            Path::new("/bin/sh"),
            vec!["-c".to_owned(), "exit 23".to_owned()],
            &temporary.path().join("exit.stderr"),
        ),
    )
    .await
    {
        Ok(_) => panic!("exited child must terminalize readiness"),
        Err(error) => error,
    };
    assert!(error.contains("reasonKind=runtime-server-candidate-exited-before-readiness"));
    assert!(error.contains("23"));
}

#[tokio::test]
async fn candidate_success_returns_typed_readiness_before_child_exit() {
    let temporary = tempfile::Builder::new()
        .prefix("asp-r-")
        .tempdir_in("/tmp")
        .expect("short candidate lifecycle fixture");
    let state_home = temporary.path().join("state");
    tokio::fs::create_dir_all(&state_home)
        .await
        .expect("create state home");
    let request_id = "candidate-success";
    let token = "candidate-token";
    let listener = listener(&state_home, request_id, token).await;
    let endpoint = listener.endpoint();
    let mut process = launch_monitored(launch_spec(
        Path::new("/bin/sh"),
        vec!["-c".to_owned(), "sleep 1".to_owned()],
        &temporary.path().join("success.stderr"),
    ))
    .await
    .expect("spawn candidate fixture");
    let process_id = process.process_id();
    let receipt = RuntimeServerReadinessReceipt {
        schema_id: "agent.semantic-protocols.runtime-server-readiness".to_owned(),
        schema_version: "1".to_owned(),
        state: RuntimeServerReadinessState::Ready,
        request_id: request_id.to_owned(),
        readiness_token: token.to_owned(),
        process_id,
        owner_epoch: 7,
        endpoint_binding_token: "candidate-binding".to_owned(),
        runtime_binary_identity:
            "blake3-256:0000000000000000000000000000000000000000000000000000000000000000".to_owned(),
        artifact_catalog_digest:
            "blake3-256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_owned(),
        transport_contract_digest:
            "blake3-256:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb".to_owned(),
        reason_kind: Some("runtime-server-ready".to_owned()),
        error: None,
    };
    let bytes = serde_json::to_vec(&receipt).expect("encode readiness receipt");
    let endpoint_path = endpoint.as_path().to_path_buf();
    let sender = tokio::spawn(async move {
        let socket = tokio::net::UnixDatagram::unbound().expect("create readiness sender");
        socket
            .send_to(&bytes, &endpoint_path)
            .await
            .expect("publish candidate readiness");
    });
    let observed = await_monitored_candidate_readiness(&listener, &mut process)
        .await
        .expect("candidate readiness terminal");
    sender.await.expect("join readiness sender");
    assert_eq!(observed, receipt);
}
