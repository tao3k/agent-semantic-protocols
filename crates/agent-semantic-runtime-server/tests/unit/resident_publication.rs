use std::path::Path;

use agent_semantic_artifacts::runtime_artifact_catalog::RuntimeBinaryIdentity;
use agent_semantic_client_db::runtime_server_control::RuntimeServerEndpoint;
use agent_semantic_client_protocol::runtime_generation::RuntimeServerGenerationIdentity;

use super::AtomicResidentPublisher;
use super::ResidentCandidateReadyReceipt;
use super::ResidentDrainAuthority;
use super::ResidentPublicationReceipt;
use super::endpoint_generation_identity;
use crate::readiness::RuntimeServerReadinessReceipt;
use crate::readiness::RuntimeServerReadinessState;

const OLD_DIGEST: &str =
    "blake3-256:1111111111111111111111111111111111111111111111111111111111111111";
const NEW_DIGEST: &str =
    "blake3-256:2222222222222222222222222222222222222222222222222222222222222222";

fn endpoint(binary: &str, owner_epoch: u64) -> RuntimeServerEndpoint {
    let schema_id = "agent.semantic-protocols.runtime-server-endpoint".to_owned();
    let schema_version = "1".to_owned();
    let transport_contract_digest = "transport".to_owned();
    let artifact_catalog_digest = "catalog".to_owned();
    let identity = RuntimeServerGenerationIdentity::derive(
        binary,
        &schema_id,
        &schema_version,
        &transport_contract_digest,
        &artifact_catalog_digest,
        owner_epoch,
    );
    RuntimeServerEndpoint {
        schema_id,
        schema_version,
        binary_content_digest: identity.binary_content_digest,
        runtime_generation_digest: identity.runtime_generation_digest,
        schema_digest: identity.schema_digest,
        transport_contract_digest,
        owner_epoch,
        owner_process_id: 7,
        runtime_artifact_path: format!("/artifacts/{binary}/asp"),
        runtime_binary_identity: RuntimeBinaryIdentity::Content {
            digest:
                agent_semantic_artifacts::blake3_content_digest::Blake3ContentDigest::parse(
                    binary,
                )
                .expect("fixture Runtime binary digest"),
        },
        monitor_capability: true,
        observed_runtime_binary_identity: RuntimeBinaryIdentity::Content {
            digest:
                agent_semantic_artifacts::blake3_content_digest::Blake3ContentDigest::parse(
                    binary,
                )
                .expect("fixture Runtime binary digest"),
        },
        artifact_mode: "release".to_owned(),
        artifact_catalog_digest,
        binding_token: format!("binding-{owner_epoch}"),
        control_endpoint: agent_semantic_client_db::runtime_server_control::RuntimeServerLoopbackEndpoint::from_socket_addr(([127, 0, 0, 1], 45001 + (owner_epoch % 100) as u16).into()).expect("control endpoint"),
        data_endpoint: agent_semantic_client_db::runtime_server_control::RuntimeServerLoopbackEndpoint::from_socket_addr(([127, 0, 0, 1], 45101 + (owner_epoch % 100) as u16).into()).expect("data endpoint"),
        provider_endpoint: agent_semantic_client_db::runtime_server_control::RuntimeServerLoopbackEndpoint::from_socket_addr(([127, 0, 0, 1], 45201 + (owner_epoch % 100) as u16).into()).expect("provider endpoint"),
        workspace_store_path: "/tmp/workspaces".to_owned(),
        status_memory_path: format!("/tmp/{owner_epoch}.status"),
    }
}

fn readiness(
    endpoint: &RuntimeServerEndpoint,
    state: RuntimeServerReadinessState,
) -> ResidentCandidateReadyReceipt {
    ResidentCandidateReadyReceipt {
        identity: endpoint_generation_identity(endpoint),
        readiness: RuntimeServerReadinessReceipt {
            schema_id: "agent.semantic-protocols.runtime-server-readiness".to_owned(),
            schema_version: "1".to_owned(),
            state,
            request_id: "request".to_owned(),
            readiness_token: "readiness".to_owned(),
            process_id: endpoint.owner_process_id,
            owner_epoch: endpoint.owner_epoch,
            endpoint_binding_token: endpoint.binding_token.clone(),
            runtime_binary_identity: match &endpoint.runtime_binary_identity {
                RuntimeBinaryIdentity::Content { digest } => digest.as_str().to_owned(),
            },
            artifact_catalog_digest: endpoint.artifact_catalog_digest.clone(),
            transport_contract_digest: endpoint.transport_contract_digest.clone(),
            reason_kind: Some("runtime-server-ready".to_owned()),
            error: None,
        },
    }
}

async fn stage(
    publisher: &AtomicResidentPublisher,
    root: &Path,
    binary: &str,
    owner_epoch: u64,
) -> (RuntimeServerEndpoint, ResidentPublicationReceipt) {
    let artifact = root.join(format!("artifact-{binary}"));
    tokio::fs::write(&artifact, binary).await.unwrap();
    let endpoint = endpoint(binary, owner_epoch);
    let candidate = publisher
        .stage_candidate(&artifact, &endpoint)
        .await
        .unwrap();
    (endpoint, candidate)
}

#[tokio::test]
async fn candidate_not_ready_does_not_switch_active_authority() {
    let root = tempfile::tempdir().unwrap();
    let publisher = AtomicResidentPublisher::new(root.path().join("resident"));
    let (endpoint, candidate) = stage(&publisher, root.path(), NEW_DIGEST, 2).await;
    let error = publisher
        .publish_ready(
            &candidate,
            &readiness(&endpoint, RuntimeServerReadinessState::Starting),
        )
        .await
        .unwrap_err();
    assert!(error.contains("not ready"));
    assert!(publisher.active().await.unwrap().is_none());
}

#[tokio::test]
async fn ready_switch_is_atomic_and_healthy_retains_previous() {
    let root = tempfile::tempdir().unwrap();
    let publisher = AtomicResidentPublisher::new(root.path().join("resident"));
    let (old_endpoint, old_candidate) = stage(&publisher, root.path(), OLD_DIGEST, 1).await;
    publisher
        .publish_ready(
            &old_candidate,
            &readiness(&old_endpoint, RuntimeServerReadinessState::Ready),
        )
        .await
        .unwrap();
    let (new_endpoint, new_candidate) = stage(&publisher, root.path(), NEW_DIGEST, 2).await;
    publisher
        .publish_ready(
            &new_candidate,
            &readiness(&new_endpoint, RuntimeServerReadinessState::Ready),
        )
        .await
        .unwrap();
    assert_eq!(
        publisher
            .active()
            .await
            .unwrap()
            .unwrap()
            .identity
            .binary_content_digest,
        NEW_DIGEST
    );
    assert_eq!(
        publisher
            .healthy()
            .await
            .unwrap()
            .unwrap()
            .identity
            .binary_content_digest,
        OLD_DIGEST
    );
}

#[tokio::test]
async fn old_generation_drains_only_after_inflight_request_releases() {
    let authority = ResidentDrainAuthority::new();
    let lease = authority.admit().unwrap();
    let draining = authority.clone();
    let task = tokio::spawn(async move {
        draining.drain().await;
    });
    tokio::task::yield_now().await;
    assert!(!task.is_finished());
    drop(lease);
    task.await.unwrap();
    assert!(authority.admit().is_err());
}
