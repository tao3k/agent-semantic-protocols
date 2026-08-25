use std::sync::Arc;
use std::time::Duration;

use agent_semantic_client_db::WorkspaceDbRegistry;
use agent_semantic_client_db::runtime_server::{RuntimeServer, RuntimeServerExit};
use agent_semantic_client_db::runtime_server_control::{
    RuntimeServerControlRequest, RuntimeServerEndpoint, RuntimeServerOperation, RuntimeServerState,
    call_runtime_server, cleanup_runtime_server_endpoint, prepare_runtime_server_endpoint,
    prepare_runtime_server_endpoint_in, prewarm_runtime_server_status_memory,
    publish_runtime_server_endpoint, runtime_server_endpoint_path,
    runtime_server_status_memory_metrics, runtime_server_transport_contract_digest,
};

pub(super) fn record_admission_fixture_candidate(project_root: &std::path::Path) {
    let digest = format!(
        "blake3:{}",
        blake3::hash(project_root.as_os_str().as_encoded_bytes()).to_hex()
    );
    agent_semantic_client_db::runtime_server_admission::record_workspace_generation_candidate(
        project_root.to_path_buf(),
        agent_semantic_client_db::runtime_server_admission::WorkspaceGenerationCandidateIdentity {
            candidate_generation: agent_semantic_runtime::git::RepositoryCandidateGeneration {
                algorithm: "blake3-worktree-state-v1".to_owned(),
                digest: digest.clone(),
                authorities: vec![
                    agent_semantic_runtime::git::RepositoryCandidateAuthority::GitIndex,
                ],
            },
            policy_overlay_digest: digest,
        },
    )
    .expect("record atomic admission fixture candidate");
}

#[test]
fn runtime_transport_identity_binds_control_workspace_and_provider_planes() {
    let domain = b"agent.semantic-protocols.runtime-server-transport";
    let control = include_bytes!("../../../../schemas/runtime-server-control.v1.schema.json");
    let data_plane = include_bytes!("../../../../schemas/workspace-db-owner-ipc.v1.schema.json");
    let performance_observation =
        include_bytes!("../../../../schemas/runtime-server-performance-observation.v1.schema.json");
    let performance_ingress_receipt = include_bytes!(
        "../../../../schemas/runtime-server-performance-ingress-receipt.v1.schema.json"
    );
    let provider_register_request =
        include_bytes!("../../../../schemas/provider-register-request.schema.json");
    let provider_register_response =
        include_bytes!("../../../../schemas/provider-register-response.schema.json");
    let mut expected = blake3::Hasher::new();
    expected.update(domain);
    for (contract_name, contract_bytes) in [
        (b"runtime-server-control".as_slice(), control.as_slice()),
        (b"workspace-db-owner-ipc".as_slice(), data_plane.as_slice()),
        (
            b"runtime-server-performance-observation".as_slice(),
            performance_observation.as_slice(),
        ),
        (
            b"runtime-server-performance-ingress-receipt".as_slice(),
            performance_ingress_receipt.as_slice(),
        ),
        (
            b"provider-register-request".as_slice(),
            provider_register_request.as_slice(),
        ),
        (
            b"provider-register-response".as_slice(),
            provider_register_response.as_slice(),
        ),
    ] {
        expected.update(&(contract_name.len() as u64).to_le_bytes());
        expected.update(contract_name);
        expected.update(&(contract_bytes.len() as u64).to_le_bytes());
        expected.update(contract_bytes);
    }
    let expected = format!("blake3-256:{}", expected.finalize().to_hex());

    assert_eq!(runtime_server_transport_contract_digest(), expected);
    assert_ne!(
        runtime_server_transport_contract_digest(),
        format!("blake3-256:{}", blake3::hash(control).to_hex()),
        "control-only identity would admit an incompatible workspace data plane"
    );
    assert_ne!(
        runtime_server_transport_contract_digest(),
        format!("blake3-256:{}", blake3::hash(data_plane).to_hex()),
        "data-plane-only identity would admit an incompatible control plane"
    );
}

pub(super) async fn fixture_endpoint(
    runtime_dir: &tempfile::TempDir,
    epoch: u64,
) -> (
    agent_semantic_client_db::RuntimeServerEndpoint,
    Arc<agent_semantic_artifacts::runtime_artifact_catalog::RuntimeArtifactCatalog>,
) {
    let state_home = agent_semantic_runtime::resolve_state_home().expect("resolve State Home");
    let catalog =
        agent_semantic_artifacts::runtime_artifact_catalog::load_runtime_artifact_catalog(
            &state_home,
        )
        .await
        .expect("load runtime artifact catalog");
    let endpoint = prepare_runtime_server_endpoint_in(
        runtime_dir.path(),
        std::path::Path::new("/runtime/asp"),
        "runtime-digest",
        catalog.mode_label(),
        &catalog.digest(),
        epoch,
        &format!("binding-{epoch}"),
    )
    .await
    .expect("prepare isolated runtime server endpoint");
    (endpoint, Arc::new(catalog))
}

async fn concurrent_runtime_status_wave(
    endpoint: &RuntimeServerEndpoint,
    request_count: usize,
    request_prefix: &'static str,
) -> Vec<Duration> {
    let barrier = Arc::new(tokio::sync::Barrier::new(request_count + 1));
    let mut clients = tokio::task::JoinSet::new();
    for index in 0..request_count {
        let endpoint = endpoint.clone();
        let barrier = Arc::clone(&barrier);
        clients.spawn(async move {
            barrier.wait().await;
            let started = tokio::time::Instant::now();
            let receipt = call_runtime_server(
                &endpoint,
                RuntimeServerOperation::Status,
                endpoint.runtime_binary_identity.clone(),
                format!("{request_prefix}-{index}"),
            )
            .await
            .expect("call concurrent runtime server");
            (receipt, started.elapsed())
        });
    }
    barrier.wait().await;

    let mut latencies = Vec::with_capacity(request_count);
    while let Some(completed) = clients.join_next().await {
        let (receipt, latency) = completed.expect("join concurrent client");
        assert_eq!(receipt.state, RuntimeServerState::Healthy);
        latencies.push(latency);
    }
    latencies.sort_unstable();
    latencies
}

#[path = "runtime_server_control/election.rs"]
mod election;
#[path = "runtime_server_control/generation_admission.rs"]
mod generation_admission;
#[path = "runtime_server_control/performance.rs"]
mod performance;
#[path = "runtime_server_control/server_lifecycle.rs"]
mod server_lifecycle;
#[path = "runtime_server_control/transport.rs"]
mod transport;

#[path = "runtime_server_control/shared_admission.rs"]
mod shared_admission;
