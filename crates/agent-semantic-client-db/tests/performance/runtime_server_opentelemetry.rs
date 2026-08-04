use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use agent_semantic_client_db::runtime_server_opentelemetry::{
    RuntimePerformanceObservation, RuntimePerformanceQuery, RuntimeServerOpenTelemetry,
    TursoOpenTelemetrySpanExporter, query_runtime_performance, try_emit_to_runtime,
};

fn short_unix_socket_path(label: &str) -> std::path::PathBuf {
    std::path::Path::new("/tmp").join(format!(
        "asp-otel-{label}-{}-{}.sock",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("fixture clock should follow Unix epoch")
            .as_nanos()
    ))
}

#[test]
fn sync_failure_adapter_does_not_require_a_tokio_reactor() {
    let fixture_root = std::env::temp_dir().join(format!(
        "asp-runtime-otel-adapter-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("fixture clock should follow Unix epoch")
            .as_nanos()
    ));
    std::fs::create_dir_all(&fixture_root).expect("adapter fixture root should exist");
    let socket_path = short_unix_socket_path("adapter");
    let receiver = std::os::unix::net::UnixDatagram::bind(&socket_path)
        .expect("adapter fixture receiver should bind");
    receiver
        .set_read_timeout(Some(Duration::from_millis(100)))
        .expect("adapter fixture receiver should be bounded");
    let observation = RuntimePerformanceObservation::new(
        "query",
        "resident-exact-generation-open",
        801_000,
        800_000,
        "budget-exceeded",
    );
    assert!(try_emit_to_runtime(&socket_path, &observation));
    let mut packet = [0_u8; 4_096];
    let received = receiver
        .recv(&mut packet)
        .expect("adapter fixture should receive one observation");
    let decoded: serde_json::Value =
        serde_json::from_slice(&packet[..received]).expect("observation should be JSON");
    assert_eq!(
        decoded.get("schemaId").and_then(serde_json::Value::as_str),
        Some("agent.semantic-protocols.runtime-server-performance-observation")
    );
    drop(receiver);
    let _ = std::fs::remove_file(socket_path);
    let _ = std::fs::remove_dir_all(fixture_root);
}

#[test]
fn concurrent_runtime_telemetry_is_nonblocking_and_turso_queryable() {
    let worker_count = std::thread::available_parallelism()
        .map(usize::from)
        .unwrap_or(1)
        .clamp(1, 32);
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(worker_count)
        .enable_all()
        .build()
        .expect("Tokio telemetry fixture runtime should build");
    runtime.block_on(async {
        let fixture_root = std::env::temp_dir().join(format!(
            "asp-runtime-otel-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("fixture clock should follow Unix epoch")
                .as_nanos()
        ));
        tokio::fs::create_dir_all(&fixture_root)
            .await
            .expect("telemetry fixture root should exist");
        let database_path = fixture_root.join("performance.turso");
        let socket_path = short_unix_socket_path("resident");
        let query_socket_path = short_unix_socket_path("query");
        let telemetry = RuntimeServerOpenTelemetry::start(
            database_path.clone(),
            socket_path.clone(),
            query_socket_path.clone(),
        )
        .expect("resident telemetry should start without opening Turso on the caller");
        let mut wire_observation = RuntimePerformanceObservation::new(
            "workspace-canonical-materialization",
            "payload-accounting",
            801_000,
            800_000,
            "budget-exceeded",
        )
        .with_materialization_metrics(7, 6407, 42, 3_300_000, 5_900_000_000);
        wire_observation.workspace_identity = Some("workspace-telemetry-fixture".to_owned());
        let wire_contract = serde_json::to_value(wire_observation)
            .expect("performance observation should serialize");
        assert_eq!(
            wire_contract
                .get("schemaId")
                .and_then(serde_json::Value::as_str),
            Some("agent.semantic-protocols.runtime-server-performance-observation")
        );
        assert_eq!(
            wire_contract
                .get("schemaVersion")
                .and_then(serde_json::Value::as_str),
            Some("1")
        );
        assert_eq!(
            wire_contract
                .get("projectionBytes")
                .and_then(serde_json::Value::as_u64),
            Some(5_900_000_000)
        );
        assert_eq!(
            wire_contract
                .get("selectorCount")
                .and_then(serde_json::Value::as_u64),
            Some(6407)
        );
        drop(
            tokio::net::UnixStream::connect(&query_socket_path)
                .await
                .expect("telemetry liveness probe should connect"),
        );
        tokio::task::yield_now().await;
        let empty_receipt = query_runtime_performance(
            &query_socket_path,
            &RuntimePerformanceQuery::new(
                "workspace-telemetry-fixture",
                "exact-query",
                "resident-exact-generation-open",
            ),
        )
        .await
        .expect("resident telemetry query should reuse its reader lane");
        assert_eq!(empty_receipt.budget_failure_count, 0);
        assert!(empty_receipt.latest_attributes_json.is_none());
        let handle = telemetry.handle();
        let observations_per_worker = 64;
        let submitted = worker_count.saturating_mul(observations_per_worker);
        let start_barrier = std::sync::Arc::new(tokio::sync::Barrier::new(worker_count));
        let mut workers = Vec::with_capacity(worker_count);
        for worker_index in 0..worker_count {
            let worker_handle = handle.clone();
            let worker_barrier = std::sync::Arc::clone(&start_barrier);
            workers.push(tokio::spawn(async move {
                worker_barrier.wait().await;
                let mut accepted = 0_u64;
                let mut latencies = Vec::with_capacity(observations_per_worker);
                for sequence in 0..observations_per_worker {
                    let started = Instant::now();
                    let mut observation = RuntimePerformanceObservation::new(
                        "exact-query",
                        "resident-exact-generation-open",
                        801_000,
                        800_000,
                        "budget-exceeded",
                    );
                    observation.workspace_identity = Some("workspace-telemetry-fixture".to_owned());
                    observation.language_id = Some("rust".to_owned());
                    observation.generation_digest =
                        Some(format!("fixture-generation-{worker_index}-{sequence}"));
                    observation.runtime_artifact_digest = Some("fixture-runtime".to_owned());
                    observation.transport_contract_digest = Some("fixture-transport".to_owned());
                    observation.failure_reason = Some("fixture-budget-exceeded".to_owned());
                    let was_accepted = worker_handle.try_record(observation);
                    latencies.push(started.elapsed());
                    accepted += u64::from(was_accepted);
                }
                (accepted, latencies)
            }));
        }
        let mut accepted = 0_u64;
        let mut enqueue_latencies = Vec::with_capacity(submitted);
        for worker in workers {
            let (worker_accepted, worker_latencies) = worker
                .await
                .expect("telemetry pressure worker should finish");
            accepted += worker_accepted;
            enqueue_latencies.extend(worker_latencies);
        }
        let dropped = handle.dropped_observation_count();
        assert_eq!(
            accepted + dropped,
            submitted as u64,
            "the bounded ingress must account for every observation"
        );
        enqueue_latencies.sort_unstable();
        let p99_index = enqueue_latencies
            .len()
            .saturating_mul(99)
            .saturating_div(100)
            .min(enqueue_latencies.len().saturating_sub(1));
        assert!(
            enqueue_latencies[p99_index] < Duration::from_millis(1),
            "telemetry ingress p99 exceeded the sub-millisecond gate: {:?}",
            enqueue_latencies[p99_index]
        );
        let populated_receipt = query_runtime_performance(
            &query_socket_path,
            &RuntimePerformanceQuery::new(
                "workspace-telemetry-fixture",
                "exact-query",
                "resident-exact-generation-open",
            ),
        )
        .await
        .expect("resident telemetry query should expose latest attributes");
        assert!(
            populated_receipt.budget_failure_count > 0
                && populated_receipt.budget_failure_count <= accepted,
            "live query must expose a monotonic persisted prefix"
        );
        let latest_attributes: serde_json::Value = serde_json::from_str(
            populated_receipt
                .latest_attributes_json
                .as_deref()
                .expect("latest attributes should be present"),
        )
        .expect("latest attributes should remain valid JSON");
        assert_eq!(
            latest_attributes
                .get("asp.performance.elapsed")
                .and_then(serde_json::Value::as_u64),
            Some(801_000)
        );
        telemetry
            .shutdown()
            .await
            .expect("resident telemetry should drain accepted observations");
        drop(handle);
        let reader = TursoOpenTelemetrySpanExporter::open(&database_path)
            .await
            .expect("telemetry Turso reader should open");
        let persisted = reader
            .budget_failure_count_for_workspace(
                "workspace-telemetry-fixture",
                "exact-query",
                "resident-exact-generation-open",
            )
            .await
            .expect("persisted budget failures should be queryable");
        assert_eq!(persisted, accepted);
        drop(reader);
        let _ = tokio::fs::remove_file(socket_path).await;
        let _ = tokio::fs::remove_file(query_socket_path).await;
        let _ = tokio::fs::remove_dir_all(fixture_root).await;
    });
}
