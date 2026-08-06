use std::sync::{Arc, RwLock};

use crate::{RuntimeServerAgentSessionLifecycleState, RuntimeServerAgentSessionStatus};

use super::{
    RuntimeServerEndpoint, RuntimeServerState, RuntimeServerStatusMemoryWriter,
    read_runtime_server_agent_sessions, read_runtime_server_cached_health_status,
    read_runtime_server_status, resolve_runtime_server_agent_session_status,
};

fn fixture_endpoint(root: &std::path::Path, owner_epoch: u64) -> RuntimeServerEndpoint {
    RuntimeServerEndpoint {
        schema_id: "agent.semantic-protocols.runtime-server-endpoint.v1".to_owned(),
        schema_version: "1".to_owned(),
        transport_contract_digest: super::super::runtime_server_transport_contract_digest(),
        owner_epoch,
        runtime_artifact_path: "/runtime/asp".to_owned(),
        runtime_artifact_digest: format!("runtime-{owner_epoch}"),
        artifact_mode: "dev".to_owned(),
        artifact_catalog_digest: format!("blake3-256:{}", "a".repeat(64)),
        binding_token: format!("binding-{owner_epoch}"),
        socket_path: root.join("control.sock").to_string_lossy().into_owned(),
        data_plane_socket_path: root.join("data.sock").to_string_lossy().into_owned(),
        workspace_store_path: root.join("workspaces").to_string_lossy().into_owned(),
        status_memory_path: root.join("status.memory").to_string_lossy().into_owned(),
    }
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
fn invalid_or_expired_projection_requires_archiving_before_replacement() {
    for lifecycle_state in [
        RuntimeServerAgentSessionLifecycleState::Invalid,
        RuntimeServerAgentSessionLifecycleState::Expired,
    ] {
        let mut session = session_status(
            "workspace-1",
            "/project-1",
            "root-1",
            "session-1",
            "asp_explorer",
            8,
            false,
        );
        session.lifecycle_state = lifecycle_state;
        let state = resolve_runtime_server_agent_session_status(
            &[session],
            "workspace-1",
            Some("session-1"),
            Some("root-1"),
            "asp_explorer",
        )
        .expect("stale status resolves to a lifecycle action");

        assert_eq!(state.state, "archive-required");
        assert_eq!(state.generation, 8);
    }
}

#[test]
fn archived_projection_requires_a_new_generation() {
    let mut session = session_status(
        "workspace-1",
        "/project-1",
        "root-1",
        "session-1",
        "asp_explorer",
        9,
        false,
    );
    session.lifecycle_state = RuntimeServerAgentSessionLifecycleState::Archived;
    let state = resolve_runtime_server_agent_session_status(
        &[session],
        "workspace-1",
        Some("session-1"),
        Some("root-1"),
        "asp_explorer",
    )
    .expect("archived status resolves to replacement");

    assert_eq!(state.state, "archived");
    assert_eq!(state.generation, 9);
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
                    || receipt.runtime_artifact_digest != "runtime-2"
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
