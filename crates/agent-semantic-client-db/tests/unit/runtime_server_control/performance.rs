use super::{
    Arc, Duration, RuntimeServer, RuntimeServerExit, RuntimeServerOperation, RuntimeServerState,
    WorkspaceDbRegistry, call_runtime_server, concurrent_runtime_status_wave, fixture_endpoint,
    prewarm_runtime_server_status_memory, runtime_server_status_memory_metrics,
};

#[tokio::test(flavor = "multi_thread")]
async fn adaptive_concurrent_runtime_control_is_sub_millisecond_at_p99() {
    let _performance = crate::test_support::performance_lock();
    let request_count = std::thread::available_parallelism()
        .map(usize::from)
        .unwrap_or(1)
        .saturating_mul(16)
        .clamp(32, 512);
    let runtime_dir = tempfile::tempdir().expect("create isolated runtime server directory");
    let (endpoint, artifact_catalog) = fixture_endpoint(&runtime_dir, 24).await;
    let server = RuntimeServer::bind_with_catalog(
        endpoint.clone(),
        Arc::new(WorkspaceDbRegistry::default()),
        artifact_catalog,
    )
    .await
    .expect("bind runtime server");
    let server = tokio::spawn(server.serve());

    let admission_started = tokio::time::Instant::now();
    let admission_metrics = prewarm_runtime_server_status_memory(&endpoint)
        .await
        .expect("admit Runtime Server status mapping before request execution");
    let admission_latency = admission_started.elapsed();
    assert_eq!(admission_metrics.reader_open_count, 1);
    assert_eq!(admission_metrics.snapshot_decode_count, 1);

    let cold_latencies =
        concurrent_runtime_status_wave(&endpoint, request_count, "adaptive-expand").await;
    let p99_index = request_count.saturating_mul(99).div_ceil(100) - 1;
    let cold_p99 = cold_latencies[p99_index];
    assert!(
        cold_p99 < Duration::from_millis(1),
        "{request_count} concurrent cold query requests exceeded sub-millisecond p99: {cold_p99:?}"
    );
    let cold_metrics =
        runtime_server_status_memory_metrics(&endpoint).expect("cold status memory metrics");
    assert_eq!(cold_metrics.reader_open_count, 1);
    assert!(
        cold_metrics.snapshot_decode_count
            <= admission_metrics.snapshot_decode_count.saturating_add(1),
        "the starting-to-healthy transition may decode one new generation, but no more: \
         admission={admission_metrics:?} cold={cold_metrics:?}"
    );

    let latencies = concurrent_runtime_status_wave(&endpoint, request_count, "adaptive-warm").await;
    let p99 = latencies[p99_index];
    assert!(
        p99 < Duration::from_millis(1),
        "{request_count} concurrent warm runtime requests exceeded sub-millisecond p99: {p99:?}"
    );
    let warm_metrics =
        runtime_server_status_memory_metrics(&endpoint).expect("warm status memory metrics");
    assert_eq!(warm_metrics.reader_open_count, 1);
    assert_eq!(
        warm_metrics.snapshot_decode_count, cold_metrics.snapshot_decode_count,
        "a stable healthy generation must not be decoded again"
    );
    assert_eq!(
        warm_metrics.snapshot_cache_hit_count - cold_metrics.snapshot_cache_hit_count,
        request_count as u64,
        "every warm Status request must use the mapped snapshot cache"
    );
    eprintln!(
        "runtime-server-performance requestCount={request_count} admissionMicros={} coldP99Micros={} \
         warmP99Micros={} readerOpens={} snapshotDecodes={} warmSnapshotCacheHits={}",
        admission_latency.as_micros(),
        cold_p99.as_micros(),
        p99.as_micros(),
        warm_metrics.reader_open_count,
        warm_metrics.snapshot_decode_count,
        warm_metrics.snapshot_cache_hit_count - cold_metrics.snapshot_cache_hit_count,
    );

    let restart = call_runtime_server(
        &endpoint,
        RuntimeServerOperation::Restart,
        agent_semantic_runtime::runtime_artifact_catalog::RuntimeBinaryIdentity::Content {
            value: "next-runtime-digest".to_owned(),
            algorithm: "blake3-256".to_owned(),
        },
        "pressure-drain".to_owned(),
    )
    .await
    .expect("drain runtime server fixture");
    assert_eq!(restart.state, RuntimeServerState::Draining);
    assert_eq!(
        server.await.expect("join runtime server").expect("serve"),
        RuntimeServerExit::RestartRequested
    );
    let draining_status = call_runtime_server(
        &endpoint,
        RuntimeServerOperation::Status,
        endpoint.runtime_binary_identity.clone(),
        "post-restart-status".to_owned(),
    )
    .await
    .expect("read draining status from mapped snapshot after server exit");
    assert_eq!(draining_status.state, RuntimeServerState::Draining);
    let draining_metrics =
        runtime_server_status_memory_metrics(&endpoint).expect("draining status memory metrics");
    assert_eq!(draining_metrics.reader_open_count, 1);
    assert_eq!(
        draining_metrics.snapshot_decode_count,
        warm_metrics.snapshot_decode_count.saturating_add(1),
        "the explicit healthy-to-draining transition must decode exactly one new generation"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn concurrent_tokio_shutdown_drains_the_runtime_server_once() {
    let runtime_dir = tempfile::tempdir().expect("create isolated runtime server directory");
    let (endpoint, artifact_catalog) = fixture_endpoint(&runtime_dir, 27).await;
    let server = RuntimeServer::bind_with_catalog(
        endpoint.clone(),
        Arc::new(WorkspaceDbRegistry::default()),
        artifact_catalog,
    )
    .await
    .expect("bind Runtime Server");
    let shutdown = server.shutdown_handle();
    let server = tokio::spawn(server.serve());

    let mut callers = tokio::task::JoinSet::new();
    for _ in 0..64 {
        let shutdown = shutdown.clone();
        callers.spawn(async move {
            shutdown.shutdown();
        });
    }
    while let Some(result) = callers.join_next().await {
        result.expect("join concurrent Runtime Server shutdown caller");
    }

    assert_eq!(
        server.await.expect("join Runtime Server").expect("serve"),
        RuntimeServerExit::ShutdownRequested
    );
    let draining = call_runtime_server(
        &endpoint,
        RuntimeServerOperation::Status,
        endpoint.runtime_binary_identity.clone(),
        "post-shutdown-status".to_owned(),
    )
    .await
    .expect("read draining status after Tokio shutdown");
    assert_eq!(draining.state, RuntimeServerState::Draining);
}
