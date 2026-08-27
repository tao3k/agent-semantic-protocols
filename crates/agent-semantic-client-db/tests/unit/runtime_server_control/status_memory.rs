use agent_semantic_artifacts::runtime_artifact_catalog::RuntimeBinaryIdentity;
use std::sync::{Arc, RwLock};

use crate::{
    RuntimeServerAgentSessionLifecycleState, RuntimeServerAgentSessionStatus,
    RuntimeServerResidentTransactionReceipt,
};

use super::{
    RuntimeServerEndpoint, RuntimeServerState, RuntimeServerStatusMemoryWriter,
    attach_resident_transaction, read_runtime_server_agent_sessions,
    read_runtime_server_cached_health_status, read_runtime_server_status,
    resolve_runtime_server_agent_session_status, validate_resident_transaction,
};

fn fixture_endpoint(root: &std::path::Path, owner_epoch: u64) -> RuntimeServerEndpoint {
    RuntimeServerEndpoint {
        binary_content_digest: RuntimeBinaryIdentity::from_bytes(
            format!("runtime-{owner_epoch}").as_bytes(),
        )
        .content_digest()
        .to_string(),
        runtime_generation_digest:
            "blake3-256:1111111111111111111111111111111111111111111111111111111111111111".to_owned(),
        schema_digest:
            "blake3-256:2222222222222222222222222222222222222222222222222222222222222222".to_owned(),
        schema_id: "agent.semantic-protocols.runtime-server-endpoint".to_owned(),
        schema_version: "1".to_owned(),
        transport_contract_digest: super::super::runtime_server_transport_contract_digest(),
        owner_epoch,
        owner_process_id: 0,
        runtime_artifact_path: "/runtime/asp".to_owned(),
        runtime_binary_identity: RuntimeBinaryIdentity::from_bytes(
            format!("runtime-{owner_epoch}").as_bytes(),
        ),
        monitor_capability: true,
        observed_runtime_binary_identity: RuntimeBinaryIdentity::from_bytes(
            format!("runtime-{owner_epoch}").as_bytes(),
        ),
        artifact_mode: "dev".to_owned(),
        artifact_catalog_digest: format!("blake3-256:{}", "a".repeat(64)),
        binding_token: format!("binding-{owner_epoch}"),
        socket_path: root.join("control.sock").to_string_lossy().into_owned(),
        data_plane_socket_path: root.join("data.sock").to_string_lossy().into_owned(),
        provider_plane_socket_path: root.join("providers.sock").to_string_lossy().into_owned(),
        workspace_store_path: root.join("workspaces").to_string_lossy().into_owned(),
        status_memory_path: root.join("status.memory").to_string_lossy().into_owned(),
    }
}

fn fixture_resident_transaction(
    endpoint: &RuntimeServerEndpoint,
) -> RuntimeServerResidentTransactionReceipt {
    let digest = endpoint.runtime_binary_identity.content_digest().clone();
    RuntimeServerResidentTransactionReceipt {
        schema_id: "agent.semantic-protocols.runtime-server-resident-transaction-receipt"
            .to_owned(),
        schema_version: "1".to_owned(),
        state: "ready".to_owned(),
        activation_generation: 7,
        launcher_artifact_path: endpoint.runtime_artifact_path.clone(),
        launcher_artifact_digest: digest.clone(),
        spawn_argv: vec![endpoint.runtime_artifact_path.clone(), "serve".to_owned()],
        applied_artifact_digest: digest.clone(),
        applied_activation_generation: 7,
        endpoint_owner_epoch: endpoint.owner_epoch,
        endpoint_binary_content_digest: digest,
        endpoint_runtime_generation_digest: endpoint.runtime_generation_digest.clone(),
        control_endpoint: endpoint.socket_path.clone(),
        data_endpoint: endpoint.data_plane_socket_path.clone(),
        provider_endpoint: endpoint.provider_plane_socket_path.clone(),
        previous_serving_digest: None,
        previous_owner_epoch: None,
        previous_drain_state: "not-required".to_owned(),
    }
}

#[tokio::test]
async fn public_status_without_state_home_fails_closed_before_endpoint_access() {
    let state_home = std::env::temp_dir().join("asp-status-without-state-home");
    let endpoint = fixture_endpoint(&state_home, 1);
    let error = agent_semantic_client_db::call_runtime_server(
        &endpoint,
        agent_semantic_client_db::RuntimeServerOperation::Status,
        endpoint.runtime_binary_identity.clone(),
        "status-without-state-home".to_owned(),
    )
    .await
    .expect_err("Status without canonical state-home authority must fail closed");
    assert_eq!(
        error,
        "state=runtime-server-status-not-current reasonKind=runtime-status-state-home-authority-required phase=runtime-server-control"
    );
}

#[tokio::test]
async fn public_healthy_status_rejects_missing_resident_transaction() {
    let root = tempfile::tempdir().expect("status memory fixture");
    let endpoint = fixture_endpoint(root.path(), 11);
    let receipt = super::RuntimeServerControlReceipt::healthy("status".to_owned(), &endpoint, 0);

    let error = attach_resident_transaction(root.path(), &endpoint, receipt)
        .await
        .expect_err("healthy status must require the canonical resident transaction");
    assert!(
        error.contains("resident transaction") || error.contains("receipt"),
        "unexpected missing resident transaction terminal: {error}"
    );
}

#[test]
fn resident_transaction_validator_accepts_one_v1_applied_generation() {
    let root = tempfile::tempdir().expect("status memory fixture");
    let endpoint = fixture_endpoint(root.path(), 12);
    let transaction = fixture_resident_transaction(&endpoint);

    let observed = validate_resident_transaction(&endpoint, transaction.clone())
        .expect("one canonical V1 transaction must validate");
    assert_eq!(observed, transaction);
}

#[test]
fn resident_transaction_validator_rejects_cross_generation_and_endpoint_authority() {
    let root = tempfile::tempdir().expect("status memory fixture");
    let endpoint = fixture_endpoint(root.path(), 13);

    let mut cross_generation = fixture_resident_transaction(&endpoint);
    cross_generation.applied_activation_generation += 1;
    assert!(validate_resident_transaction(&endpoint, cross_generation).is_err());

    let mut cross_endpoint = fixture_resident_transaction(&endpoint);
    cross_endpoint.endpoint_runtime_generation_digest = format!("blake3-256:{}", "f".repeat(64));
    assert!(validate_resident_transaction(&endpoint, cross_endpoint).is_err());

    let mut cross_launcher = fixture_resident_transaction(&endpoint);
    cross_launcher.launcher_artifact_path = root.path().join("other-asp").display().to_string();
    assert!(validate_resident_transaction(&endpoint, cross_launcher).is_err());
}

fn session_status(
    workspace_identity: &str,
    project_id: &str,
    root_session_id: &str,
    session_id: &str,
    name: &str,
    generation: u64,
    routable: bool,
) -> RuntimeServerAgentSessionStatus {
    let host_binding = routable.then(|| {
        serde_json::json!({
            "schemaVersion": "1",
            "rootSessionId": root_session_id,
            "hostChildId": session_id,
            "residentId": name,
            "generation": generation,
            "lifecycleState": "live",
            "routable": true,
        })
    });
    RuntimeServerAgentSessionStatus {
        workspace_identity: workspace_identity.to_owned(),
        project_id: project_id.to_owned(),
        root_session_id: root_session_id.to_owned(),
        session_id: session_id.to_owned(),
        name: name.to_owned(),
        physical_generation: generation,
        lifecycle_state: if routable {
            RuntimeServerAgentSessionLifecycleState::Routable
        } else {
            RuntimeServerAgentSessionLifecycleState::Invalid
        },
        host_binding,
    }
}

#[test]
fn empty_projection_requires_registration_for_an_observed_root() {
    let state = resolve_runtime_server_agent_session_status(
        &[],
        "workspace-1",
        None,
        Some("root-1"),
        "asp_explorer",
    )
    .expect("empty projection is a valid registration-required state");
    assert_eq!(state.state, "registration-required");
    assert_eq!(state.project_id.as_deref(), Some("workspace-1"));
    assert_eq!(state.root_session_id.as_deref(), Some("root-1"));
    assert_eq!(state.generation, 0);
    assert_eq!(state.reason_kind, None);
    assert_eq!(state.host_binding, None);
}

#[test]
fn existing_unbound_namespace_is_resumable_without_replacement() {
    let mut session = session_status(
        "workspace-1",
        "/project-1",
        "root-1",
        "child-1",
        "asp_explorer",
        3,
        true,
    );
    session.host_binding = None;
    let state = resolve_runtime_server_agent_session_status(
        &[session],
        "workspace-1",
        None,
        Some("root-1"),
        "asp_explorer",
    )
    .expect("unbound matched child is a typed resumable state");
    assert_eq!(state.state, "resumable");
    assert_eq!(
        state.reason_kind.as_deref(),
        Some("registered-namespace-needs-resume")
    );
    assert_eq!(state.generation, 3);
}

#[test]
fn routable_projection_resolves_registered_generation() {
    let sessions = [session_status(
        "workspace-1",
        "/project-1",
        "root-1",
        "session-1",
        "asp_explorer",
        7,
        true,
    )];
    let state = resolve_runtime_server_agent_session_status(
        &sessions,
        "workspace-1",
        Some("session-1"),
        Some("root-1"),
        "asp_explorer",
    )
    .expect("matching status resolves");
    assert_eq!(state.state, "registered");
    assert_eq!(state.generation, 7);
}

#[test]
fn stopped_projection_resumes_the_same_durable_generation() {
    let mut session = session_status(
        "workspace-1",
        "/project-1",
        "root-1",
        "session-1",
        "asp_explorer",
        8,
        false,
    );
    session.lifecycle_state = RuntimeServerAgentSessionLifecycleState::Stopped;
    let state = resolve_runtime_server_agent_session_status(
        &[session],
        "workspace-1",
        Some("session-1"),
        Some("root-1"),
        "asp_explorer",
    )
    .expect("stopped physical child remains resumable");

    assert_eq!(state.state, "resumable");
    assert_eq!(state.generation, 8);
}

#[test]
fn achieved_projection_is_terminal_and_never_replaced() {
    let mut session = session_status(
        "workspace-1",
        "/project-1",
        "root-1",
        "session-1",
        "asp_explorer",
        9,
        false,
    );
    session.lifecycle_state = RuntimeServerAgentSessionLifecycleState::Achieved;
    let state = resolve_runtime_server_agent_session_status(
        &[session],
        "workspace-1",
        Some("session-1"),
        Some("root-1"),
        "asp_explorer",
    )
    .expect("achieved namespace resolves to terminal state");

    assert_eq!(state.state, "achieved");
    assert_eq!(state.generation, 9);
}

#[test]
fn invalid_projection_is_blocked_and_never_replaced_implicitly() {
    let mut session = session_status(
        "workspace-1",
        "/project-1",
        "root-1",
        "session-1",
        "asp_explorer",
        10,
        false,
    );
    session.lifecycle_state = RuntimeServerAgentSessionLifecycleState::Invalid;
    let state = resolve_runtime_server_agent_session_status(
        &[session],
        "workspace-1",
        Some("session-1"),
        Some("root-1"),
        "asp_explorer",
    )
    .expect("invalid namespace resolves to a fail-closed state");

    assert_eq!(state.state, "blocked");
    assert_eq!(state.generation, 10);
}

#[test]
fn observed_session_root_mismatch_fails_closed() {
    let sessions = [session_status(
        "workspace-1",
        "/project-1",
        "root-actual",
        "session-1",
        "asp_explorer",
        1,
        true,
    )];
    let error = resolve_runtime_server_agent_session_status(
        &sessions,
        "workspace-1",
        Some("session-1"),
        Some("root-reported"),
        "asp_explorer",
    )
    .expect_err("root mismatch must fail closed");
    assert!(error.contains("identity mismatch"));
}

#[test]
fn projected_workspace_mismatch_fails_closed() {
    let sessions = [session_status(
        "workspace-other",
        "/project-other",
        "root-1",
        "session-1",
        "asp_explorer",
        1,
        true,
    )];
    let error = resolve_runtime_server_agent_session_status(
        &sessions,
        "workspace-1",
        Some("session-1"),
        Some("root-1"),
        "asp_explorer",
    )
    .expect_err("workspace mismatch must fail closed");
    assert!(error.contains("workspace mismatch"));
}

#[tokio::test]
async fn status_memory_publishes_agent_session_projection() {
    let root = tempfile::tempdir().expect("status memory fixture");
    let endpoint = fixture_endpoint(root.path(), 12);
    let sessions = Arc::new(RwLock::new(vec![session_status(
        "workspace-1",
        "/project-1",
        "root-1",
        "session-1",
        "asp_explorer",
        4,
        true,
    )]));
    let mut writer = RuntimeServerStatusMemoryWriter::create(&endpoint)
        .await
        .expect("create status memory");
    writer.set_agent_sessions(Arc::clone(&sessions));
    writer
        .publish(RuntimeServerState::Healthy, 1)
        .expect("publish status projection");

    let projected = read_runtime_server_agent_sessions(&endpoint)
        .await
        .expect("read agent session projection");
    assert_eq!(
        projected,
        *sessions.read().expect("session projection lock")
    );
}

#[tokio::test]
async fn removed_status_memory_invalidates_a_cached_mapping() {
    let root = tempfile::tempdir().expect("status memory fixture");
    let endpoint = fixture_endpoint(root.path(), 1);
    let mut writer = RuntimeServerStatusMemoryWriter::create(&endpoint)
        .await
        .expect("create status memory");
    writer
        .publish(RuntimeServerState::Draining, 3)
        .expect("publish draining");
    let receipt = read_runtime_server_cached_health_status(
        std::path::Path::new(&endpoint.status_memory_path),
        "before-removal".to_owned(),
    )
    .await
    .expect("read live status memory");
    assert_eq!(receipt.state, RuntimeServerState::Draining);

    drop(writer);
    tokio::fs::remove_file(&endpoint.status_memory_path)
        .await
        .expect("remove status memory");
    let error = read_runtime_server_cached_health_status(
        std::path::Path::new(&endpoint.status_memory_path),
        "after-removal".to_owned(),
    )
    .await
    .expect_err("an unlinked status mapping must never remain authoritative");
    assert!(error.contains("failed to inspect Runtime Server status memory"));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn replacement_epoch_reopens_once_for_concurrent_sessions() {
    let root = tempfile::tempdir().expect("status memory fixture");
    let first = fixture_endpoint(root.path(), 1);
    let mut first_writer = RuntimeServerStatusMemoryWriter::create(&first)
        .await
        .expect("create first status memory");
    first_writer
        .publish(RuntimeServerState::Draining, 1)
        .expect("publish first epoch");
    read_runtime_server_cached_health_status(
        std::path::Path::new(&first.status_memory_path),
        "first-epoch".to_owned(),
    )
    .await
    .expect("cache first epoch");
    drop(first_writer);
    tokio::fs::remove_file(&first.status_memory_path)
        .await
        .expect("remove first epoch");

    let second = Arc::new(fixture_endpoint(root.path(), 2));
    let mut second_writer = RuntimeServerStatusMemoryWriter::create(&second)
        .await
        .expect("create replacement status memory");
    second_writer
        .publish(RuntimeServerState::Healthy, 7)
        .expect("publish replacement epoch");

    let mut readers = tokio::task::JoinSet::new();
    for session in 0..64 {
        let second = Arc::clone(&second);
        readers.spawn(async move {
            for sample in 0..64 {
                let receipt = read_runtime_server_status(
                    &second,
                    format!("session-{session}-sample-{sample}"),
                )
                .await?;
                if receipt.state != RuntimeServerState::Healthy
                    || receipt.runtime_binary_identity
                        != RuntimeBinaryIdentity::from_bytes(b"runtime-2")
                    || receipt.workspace_entry_count != 7
                {
                    return Err(format!("stale replacement receipt: {receipt:?}"));
                }
            }
            Ok::<(), String>(())
        });
    }
    while let Some(result) = readers.join_next().await {
        result
            .expect("reader task joins")
            .expect("reader sees new epoch");
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn cached_status_identity_check_is_sub_millisecond_at_p99() {
    let root = tempfile::tempdir().expect("status memory fixture");
    let endpoint = fixture_endpoint(root.path(), 9);
    let mut writer = RuntimeServerStatusMemoryWriter::create(&endpoint)
        .await
        .expect("create status memory");
    writer
        .publish(RuntimeServerState::Healthy, 2)
        .expect("publish healthy");
    read_runtime_server_status(&endpoint, "prewarm".to_owned())
        .await
        .expect("prewarm reader");

    let mut samples = Vec::with_capacity(10_000);
    for sample in 0..10_000 {
        let started = std::time::Instant::now();
        read_runtime_server_status(&endpoint, format!("sample-{sample}"))
            .await
            .expect("read cached status");
        samples.push(started.elapsed().as_nanos());
    }
    samples.sort_unstable();
    let p99 = samples[(samples.len() * 99) / 100];
    eprintln!(
        "[runtime-status-memory] samples={} p99Nanos={p99} filesystemIdentityChecks=0 bindingAuthority=endpoint-owner-epoch",
        samples.len()
    );
    assert!(
        p99 < 1_000_000,
        "cached status identity validation p99 must remain below 1ms; p99Nanos={p99}"
    );
}
