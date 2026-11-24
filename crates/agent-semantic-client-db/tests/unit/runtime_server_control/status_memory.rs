// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use agent_semantic_artifacts::runtime_artifact_catalog::RuntimeBinaryIdentity;
use std::sync::Arc;

use crate::RuntimeServerResidentTransactionReceipt;

use super::{
    RuntimeServerEndpoint, RuntimeServerState, RuntimeServerStatusMemoryWriter,
    attach_resident_transaction, read_runtime_server_cached_health_status,
    read_runtime_server_status, validate_resident_transaction,
};

fn loopback(port: u16) -> crate::runtime_server_control::RuntimeServerLoopbackEndpoint {
    crate::runtime_server_control::RuntimeServerLoopbackEndpoint::from_socket_addr(
        ([127, 0, 0, 1], port).into(),
    )
    .expect("loopback endpoint")
}

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
        transport_binding: super::super::RuntimeTransportBinding {
            transport_contract_digest: super::super::runtime_server_transport_contract_digest(),
            owner_epoch,
            owner_process_id: 1,
            binding_token: format!("binding-{owner_epoch}"),
            control_endpoint: loopback(42001 + (owner_epoch % 100) as u16),
            data_endpoint: loopback(42101 + (owner_epoch % 100) as u16),
            provider_endpoint: loopback(42201 + (owner_epoch % 100) as u16),
        },
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
        publication_nonce: "publication-status-memory".to_owned(),
        launcher_artifact_path: endpoint.runtime_artifact_path.clone(),
        launcher_artifact_digest: digest.clone(),
        spawn_argv: vec![endpoint.runtime_artifact_path.clone(), "serve".to_owned()],
        applied_artifact_digest: digest.clone(),
        applied_publication_nonce: "publication-status-memory".to_owned(),
        endpoint_owner_epoch: endpoint.owner_epoch,
        endpoint_binary_content_digest: digest,
        endpoint_runtime_generation_digest: endpoint.runtime_generation_digest.clone(),
        control_endpoint: endpoint.control_endpoint.clone(),
        data_endpoint: endpoint.data_endpoint.clone(),
        provider_endpoint: endpoint.provider_endpoint.clone(),
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
fn resident_transaction_validator_rejects_cross_publication_and_endpoint_authority() {
    let root = tempfile::tempdir().expect("status memory fixture");
    let endpoint = fixture_endpoint(root.path(), 13);

    let mut cross_publication = fixture_resident_transaction(&endpoint);
    cross_publication.applied_publication_nonce = "publication-other".to_owned();
    assert!(validate_resident_transaction(&endpoint, cross_publication).is_err());

    let mut cross_endpoint = fixture_resident_transaction(&endpoint);
    cross_endpoint.endpoint_runtime_generation_digest = format!("blake3-256:{}", "f".repeat(64));
    assert!(validate_resident_transaction(&endpoint, cross_endpoint).is_err());

    let mut cross_launcher = fixture_resident_transaction(&endpoint);
    cross_launcher.launcher_artifact_path = root.path().join("other-asp").display().to_string();
    assert!(validate_resident_transaction(&endpoint, cross_launcher).is_err());
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

#[tokio::test]
async fn duplicate_writer_cannot_truncate_a_live_reader_mapping() {
    let root = tempfile::tempdir().expect("status memory fixture");
    let endpoint = fixture_endpoint(root.path(), 8);
    let mut writer = RuntimeServerStatusMemoryWriter::create(&endpoint)
        .await
        .expect("create status memory");
    writer
        .publish(RuntimeServerState::Healthy, 5)
        .expect("publish healthy status");
    let before = read_runtime_server_status(&endpoint, "before-duplicate".to_owned())
        .await
        .expect("map live status memory");
    assert_eq!(before.state, RuntimeServerState::Healthy);

    let error = match RuntimeServerStatusMemoryWriter::create(&endpoint).await {
        Ok(_) => panic!("same generation and binding must reject a second writer"),
        Err(error) => error,
    };
    assert!(
        error.contains("failed to create Runtime Server status memory"),
        "{error}"
    );

    let after = read_runtime_server_status(&endpoint, "after-duplicate".to_owned())
        .await
        .expect("live mapping remains readable after rejected duplicate");
    assert_eq!(after.state, RuntimeServerState::Healthy);
    assert_eq!(after.workspace_entry_count, 5);
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
