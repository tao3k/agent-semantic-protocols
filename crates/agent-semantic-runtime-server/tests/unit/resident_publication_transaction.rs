use super::AtomicResidentPublisher;
use super::ResidentCandidateReadyReceipt;
use super::publish_candidate_transaction;
use crate::readiness::RuntimeServerReadinessReceipt;
use crate::readiness::RuntimeServerReadinessState;
use agent_semantic_artifacts::runtime_artifact_catalog::RuntimeBinaryIdentity;
use agent_semantic_client_db::runtime_server_control::RuntimeServerEndpoint;
use agent_semantic_client_db::runtime_server_control::runtime_server_transport_contract_digest;
use agent_semantic_client_protocol::runtime_generation::RuntimeServerGenerationIdentity;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;

#[tokio::test]
async fn production_transaction_switches_only_after_ready_and_invokes_previous_drain() {
    let temporary = tempfile::tempdir().expect("temporary resident publication");
    let binary = temporary.path().join("candidate-asp");
    tokio::fs::write(&binary, b"resident-candidate")
        .await
        .expect("write candidate binary");
    let binary_content_digest =
        "blake3-256:0000000000000000000000000000000000000000000000000000000000000000".to_owned();
    let artifact_catalog_digest =
        "blake3-256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_owned();
    let transport_contract_digest = runtime_server_transport_contract_digest();
    let derived_identity = RuntimeServerGenerationIdentity::derive(
        binary_content_digest.clone(),
        "agent.semantic-protocols.runtime-server-endpoint",
        "1",
        &transport_contract_digest,
        &artifact_catalog_digest,
        7,
    );
    let runtime_generation_digest = derived_identity.runtime_generation_digest.clone();
    let schema_digest = derived_identity.schema_digest.clone();
    let endpoint = RuntimeServerEndpoint {
        schema_id: "agent.semantic-protocols.runtime-server-endpoint".to_owned(),
        schema_version: "1".to_owned(),
        binary_content_digest: binary_content_digest.clone(),
        runtime_generation_digest: runtime_generation_digest.clone(),
        schema_digest: schema_digest.clone(),
        transport_contract_digest: transport_contract_digest.clone(),
        owner_epoch: 7,
        owner_process_id: 77,
        runtime_artifact_path: binary.to_string_lossy().into_owned(),
        runtime_binary_identity: RuntimeBinaryIdentity::Content {
            digest:
                agent_semantic_artifacts::blake3_content_digest::Blake3ContentDigest::parse(
                    &binary_content_digest,
                )
                .expect("fixture Runtime binary digest"),
        },
        monitor_capability: true,
        observed_runtime_binary_identity: RuntimeBinaryIdentity::Content {
            digest:
                agent_semantic_artifacts::blake3_content_digest::Blake3ContentDigest::parse(
                    &binary_content_digest,
                )
                .expect("fixture Runtime binary digest"),
        },
        artifact_mode: "release".to_owned(),
        artifact_catalog_digest: artifact_catalog_digest.clone(),
        binding_token: "candidate-binding".to_owned(),
        control_endpoint: agent_semantic_client_db::runtime_server_control::RuntimeServerLoopbackEndpoint::from_socket_addr(([127, 0, 0, 1], 45301).into()).expect("control endpoint"),
        data_endpoint: agent_semantic_client_db::runtime_server_control::RuntimeServerLoopbackEndpoint::from_socket_addr(([127, 0, 0, 1], 45302).into()).expect("data endpoint"),
        provider_endpoint: agent_semantic_client_db::runtime_server_control::RuntimeServerLoopbackEndpoint::from_socket_addr(([127, 0, 0, 1], 45303).into()).expect("provider endpoint"),
        workspace_store_path: "/runtime/workspaces".to_owned(),
        status_memory_path: "/runtime/status.memory".to_owned(),
    };
    let ready = ResidentCandidateReadyReceipt {
        identity: derived_identity,
        readiness: RuntimeServerReadinessReceipt {
            schema_id: "agent.semantic-protocols.runtime-server-readiness".to_owned(),
            schema_version: "1".to_owned(),
            state: RuntimeServerReadinessState::Ready,
            request_id: "resident-transaction-test".to_owned(),
            readiness_token: "resident-transaction-token".to_owned(),
            process_id: endpoint.owner_process_id,
            owner_epoch: endpoint.owner_epoch,
            endpoint_binding_token: endpoint.binding_token.clone(),
            runtime_binary_identity: endpoint.binary_content_digest.clone(),
            artifact_catalog_digest,
            transport_contract_digest,
            reason_kind: Some("runtime-server-ready".to_owned()),
            error: None,
        },
    };
    let drain_called = Arc::new(AtomicBool::new(false));
    let drain_observer = Arc::clone(&drain_called);
    let publisher = AtomicResidentPublisher::new(temporary.path().join("resident"));
    let mut endpoint = endpoint;
    let _control_listener = tokio::net::TcpListener::bind(("127.0.0.1", 0))
        .await
        .expect("bind candidate control plane");
    let _provider_listener = tokio::net::TcpListener::bind(("127.0.0.1", 0))
        .await
        .expect("bind candidate provider plane");
    endpoint.control_endpoint = agent_semantic_client_db::runtime_server_control::RuntimeServerLoopbackEndpoint::from_socket_addr(_control_listener.local_addr().expect("control address")).expect("control endpoint");
    endpoint.provider_endpoint = agent_semantic_client_db::runtime_server_control::RuntimeServerLoopbackEndpoint::from_socket_addr(_provider_listener.local_addr().expect("provider address")).expect("provider endpoint");
    let data_listener = tokio::net::TcpListener::bind(("127.0.0.1", 0))
        .await
        .expect("reserve candidate data plane");
    endpoint.data_endpoint = agent_semantic_client_db::runtime_server_control::RuntimeServerLoopbackEndpoint::from_socket_addr(data_listener.local_addr().expect("data address")).expect("data endpoint");
    drop(data_listener);

    let active_before_failure = publisher
        .active()
        .await
        .expect("read active before data-plane failure")
        .map(|slot| slot.identity);
    let healthy_before_failure = publisher
        .healthy()
        .await
        .expect("read healthy before data-plane failure")
        .map(|slot| slot.identity);
    let failed_drain_observer = drain_called.clone();
    let failure =
        publish_candidate_transaction(&publisher, &binary, &endpoint, &ready, move || async move {
            failed_drain_observer.store(true, Ordering::Release);
            Ok(())
        })
        .await
        .expect_err("missing data plane must reject resident candidate");
    assert!(
        failure.contains("data"),
        "typed failure must identify the missing data plane: {failure}"
    );
    assert_eq!(
        publisher
            .active()
            .await
            .expect("read active after data-plane failure")
            .map(|slot| slot.identity),
        active_before_failure
    );
    assert_eq!(
        publisher
            .healthy()
            .await
            .expect("read healthy after data-plane failure")
            .map(|slot| slot.identity),
        healthy_before_failure
    );
    assert!(
        !drain_called.load(Ordering::Acquire),
        "precommit data-plane failure must not drain the serving generation"
    );
    let _data_listener = tokio::net::TcpListener::bind(endpoint.data_endpoint.socket_addr())
        .await
        .expect("bind candidate data plane");

    let receipt =
        publish_candidate_transaction(&publisher, &binary, &endpoint, &ready, move || async move {
            drain_observer.store(true, Ordering::Release);
            Ok(())
        })
        .await
        .expect("publish ready resident candidate");
    assert!(drain_called.load(Ordering::Acquire));
    assert!(receipt.publication_dir.join("asp").is_file());
    assert_eq!(receipt.identity, ready.identity);

    let drain_error =
        publish_candidate_transaction(&publisher, &binary, &endpoint, &ready, || async {
            Err("old Runtime drain failed".to_owned())
        })
        .await
        .expect_err("drain failure is a typed terminal after authority commit");
    assert!(drain_error.contains("after active authority commit"));
    assert_eq!(
        publisher.active().await.unwrap().unwrap().identity,
        ready.identity
    );
    assert_eq!(
        publisher.healthy().await.unwrap().unwrap().identity,
        ready.identity
    );
}
