// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use std::sync::Arc;
use std::time::Duration;

use agent_semantic_artifacts::runtime_artifact_catalog::RuntimeArtifactCatalog;
use agent_semantic_client_db::WorkspaceDbRegistry;
use agent_semantic_client_db::runtime_server::RuntimeServer;
use agent_semantic_client_db::runtime_server_control::RuntimeServerOperation;
use agent_semantic_client_db::runtime_server_control::RuntimeServerState;
use agent_semantic_client_db::runtime_server_control::call_runtime_server;
use agent_semantic_client_db::runtime_server_control::prepare_runtime_server_endpoint_in;
use agent_semantic_client_db::runtime_server_control::runtime_server_endpoint_path;
use agent_semantic_client_db::runtime_server_health::cached_runtime_server_health;
use agent_semantic_config::runtime_dev::RuntimeArtifactMode;

#[tokio::test(flavor = "multi_thread")]
async fn concurrent_cached_health_is_sub_millisecond_at_p99() {
    let _performance = crate::test_support::performance_lock();
    let state_home = tempfile::tempdir().expect("create isolated state home");
    let catalog = Arc::new(RuntimeArtifactCatalog::new(RuntimeArtifactMode::Release));
    let endpoint = prepare_runtime_server_endpoint_in(
        state_home.path(),
        std::path::Path::new("/runtime/asp"),
        &agent_semantic_artifacts::blake3_content_digest::Blake3ContentDigest::from_bytes(
            b"runtime-digest",
        ),
        catalog.mode_label(),
        &catalog.digest(),
        1,
        "cached-health-binding",
    )
    .await
    .expect("prepare Runtime Server endpoint");
    let health_endpoint = endpoint.clone();
    let server = RuntimeServer::bind_with_catalog(
        endpoint.clone(),
        Arc::new(WorkspaceDbRegistry::default()),
        catalog,
    )
    .await
    .expect("bind Runtime Server control plane");
    let _client_grpc_listener = tokio::net::TcpListener::bind(endpoint.data_endpoint.socket_addr())
        .await
        .expect("bind ASP Client gRPC plane");
    let _provider_listener =
        tokio::net::TcpListener::bind(endpoint.provider_endpoint.socket_addr())
            .await
            .expect("bind provider plane");
    server
        .publish_endpoint_after_required_planes(
            &runtime_server_endpoint_path(state_home.path())
                .expect("derive Runtime Server endpoint path"),
        )
        .await
        .expect("publish ready Runtime Server endpoint");
    let shutdown = server.shutdown_handle();
    let server = tokio::spawn(server.serve());

    let serving = call_runtime_server(
        &health_endpoint,
        RuntimeServerOperation::Status,
        health_endpoint.runtime_binary_identity.clone(),
        "cached-health-serving-barrier".to_owned(),
    )
    .await
    .expect("synchronize with serving Runtime Server");
    assert_eq!(serving.state, RuntimeServerState::Healthy);

    let health = cached_runtime_server_health(&health_endpoint)
        .await
        .expect("admit cached health endpoint");
    assert!(health.is_healthy());

    let request_count = std::thread::available_parallelism()
        .map(usize::from)
        .unwrap_or(1)
        .saturating_mul(16)
        .clamp(32, 512);
    let mut requests = tokio::task::JoinSet::new();
    for _ in 0..request_count {
        let health_endpoint = health_endpoint.clone();
        requests.spawn(async move {
            let started = tokio::time::Instant::now();
            let health = cached_runtime_server_health(&health_endpoint)
                .await
                .expect("read cached health");
            assert!(health.is_healthy());
            started.elapsed()
        });
    }
    let mut latencies = Vec::with_capacity(request_count);
    while let Some(result) = requests.join_next().await {
        latencies.push(result.expect("join cached health request"));
    }
    latencies.sort_unstable();
    let p99 = latencies[request_count.saturating_mul(99).div_ceil(100) - 1];
    assert!(
        p99 < Duration::from_millis(1),
        "{request_count} concurrent cached health requests exceeded sub-millisecond p99: {p99:?}"
    );
    eprintln!(
        "runtime-server-cached-health requestCount={request_count} p99Micros={}",
        p99.as_micros()
    );

    shutdown.shutdown();
    server
        .await
        .expect("join Runtime Server")
        .expect("stop Runtime Server");
}
