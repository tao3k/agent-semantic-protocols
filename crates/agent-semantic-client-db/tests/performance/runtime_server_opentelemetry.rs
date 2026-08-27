use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use agent_semantic_client_db::runtime_server_opentelemetry::{
    RuntimePerformanceObservation, RuntimePerformanceQuery, RuntimeServerOpenTelemetry,
    TursoOpenTelemetrySpanExporter, admit_to_runtime, query_runtime_performance,
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

#[tokio::test(flavor = "multi_thread")]
async fn repeated_runtime_telemetry_lifecycle_returns_to_task_and_socket_baseline() {
    let _performance = crate::test_support::performance_lock();
    let baseline_alive_tasks = tokio::runtime::Handle::current()
        .metrics()
        .num_alive_tasks();

    for cycle in 0..16 {
        let fixture_root = std::env::temp_dir().join(format!(
            "asp-runtime-otel-lifecycle-{}-{cycle}",
            std::process::id()
        ));
        tokio::fs::create_dir_all(&fixture_root)
            .await
            .expect("lifecycle fixture root should exist");
        let socket_path = short_unix_socket_path("lifecycle-ingress");
        let query_socket_path = short_unix_socket_path("lifecycle-query");
        let telemetry = RuntimeServerOpenTelemetry::start(
            fixture_root.join("performance.turso"),
            socket_path.clone(),
            query_socket_path.clone(),
        )
        .await
        .expect("Telemetry lifecycle should start");
        let mut observation = RuntimePerformanceObservation::new(
            "runtime-server",
            "lifecycle-soak",
            100,
            1_000,
            "within-budget",
        );
        observation.workspace_identity = Some(format!("workspace-lifecycle-{cycle}"));
        assert!(
            telemetry.handle().try_record(observation),
            "one lifecycle observation must fit the adaptive bounded queue"
        );
        let receipt = telemetry
            .shutdown()
            .await
            .expect("Telemetry lifecycle must drain");
        assert_eq!(receipt.state, "terminated");
        assert_eq!(
            receipt.started,
            receipt.completed + receipt.cancelled + receipt.failed
        );
        assert_eq!(receipt.active, 0);
        assert_eq!(receipt.leaked, 0);
        assert!(
            !tokio::fs::try_exists(&socket_path)
                .await
                .expect("ingress socket state should be readable"),
            "Telemetry component must remove its own ingress socket"
        );
        assert!(
            !tokio::fs::try_exists(&query_socket_path)
                .await
                .expect("query socket state should be readable"),
            "Telemetry component must remove its own query socket"
        );
        tokio::fs::remove_dir_all(fixture_root)
            .await
            .expect("lifecycle fixture should be removable after shutdown");
    }

    tokio::task::yield_now().await;
    let final_alive_tasks = tokio::runtime::Handle::current()
        .metrics()
        .num_alive_tasks();
    assert!(
        final_alive_tasks <= baseline_alive_tasks.saturating_add(1),
        "repeated Telemetry lifecycle leaked Tokio tasks: baseline={baseline_alive_tasks} final={final_alive_tasks}"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn multi_workspace_multi_session_telemetry_remains_nonblocking_and_drains() {
    let _performance = crate::test_support::performance_lock();
    const WORKSPACES: usize = 5;
    const SESSIONS_PER_WORKSPACE: usize = 16;
    const OBSERVATIONS_PER_SESSION: usize = 32;
    let fixture_root = std::env::temp_dir().join(format!(
        "asp-runtime-otel-multi-workspace-{}",
        std::process::id()
    ));
    tokio::fs::create_dir_all(&fixture_root)
        .await
        .expect("multi-workspace fixture root should exist");
    let socket_path = short_unix_socket_path("multi-workspace-ingress");
    let query_socket_path = short_unix_socket_path("multi-workspace-query");
    let telemetry = RuntimeServerOpenTelemetry::start(
        fixture_root.join("performance.turso"),
        socket_path,
        query_socket_path.clone(),
    )
    .await
    .expect("multi-workspace Telemetry should start");
    let handle = telemetry.handle();
    let producer_count = WORKSPACES * SESSIONS_PER_WORKSPACE;
    let barrier = std::sync::Arc::new(tokio::sync::Barrier::new(producer_count + 1));
    let mut producers = Vec::with_capacity(producer_count);
    for workspace in 0..WORKSPACES {
        for session in 0..SESSIONS_PER_WORKSPACE {
            let handle = handle.clone();
            let barrier = std::sync::Arc::clone(&barrier);
            producers.push(tokio::spawn(async move {
                barrier.wait().await;
                let mut accepted = 0_u64;
                let mut latencies = Vec::with_capacity(OBSERVATIONS_PER_SESSION);
                for sequence in 0..OBSERVATIONS_PER_SESSION {
                    let mut observation = RuntimePerformanceObservation::new(
                        "query",
                        "multi-workspace-read-only",
                        100,
                        1_000,
                        "within-budget",
                    );
                    observation.workspace_identity = Some(format!("workspace-{workspace}"));
                    observation.operation_id =
                        Some(format!("session-{session}-observation-{sequence}"));
                    let started = Instant::now();
                    accepted += u64::from(handle.try_record(observation));
                    latencies.push(started.elapsed());
                }
                (accepted, latencies)
            }));
        }
    }
    barrier.wait().await;
    let mut accepted = 0_u64;
    let mut enqueue_latencies = Vec::with_capacity(producer_count * OBSERVATIONS_PER_SESSION);
    for producer in producers {
        let (producer_accepted, producer_latencies) = producer
            .await
            .expect("multi-workspace producer should join");
        accepted += producer_accepted;
        enqueue_latencies.extend(producer_latencies);
    }
    let submitted = u64::try_from(producer_count * OBSERVATIONS_PER_SESSION)
        .expect("submitted observation count should fit u64");
    assert_eq!(
        accepted + handle.dropped_observation_count(),
        submitted,
        "bounded Telemetry ingress must account for all sessions and workspaces"
    );
    enqueue_latencies.sort_unstable();
    let samples = enqueue_latencies.len();
    let p50 = enqueue_latencies[samples * 50 / 100];
    let p95 = enqueue_latencies[samples * 95 / 100];
    let p99 = enqueue_latencies[enqueue_latencies.len() * 99 / 100];
    let max = *enqueue_latencies
        .last()
        .expect("Telemetry performance receipt requires at least one sample");
    eprintln!(
        "runtime-telemetry-enqueue samples={samples} p50Micros={} p95Micros={} p99Micros={} maxMicros={}",
        p50.as_micros(),
        p95.as_micros(),
        p99.as_micros(),
        max.as_micros()
    );
    assert!(
        p99 < Duration::from_millis(1),
        "multi-workspace Telemetry enqueue p99 must remain sub-millisecond: {p99:?}"
    );

    tokio::task::yield_now().await;
    let mut visible_prefix = 0_u64;
    for workspace in 0..WORKSPACES {
        let receipt = query_runtime_performance(
            &query_socket_path,
            &RuntimePerformanceQuery::new(
                format!("workspace-{workspace}"),
                "query",
                "multi-workspace-read-only",
            ),
        )
        .await
        .expect("resident live query must not wait for a Turso flush");
        visible_prefix = visible_prefix.saturating_add(receipt.observation_count);
        assert_eq!(receipt.budget_failure_count, 0);
        if receipt.observation_count > 0 {
            assert_eq!(receipt.p99_micros, Some(100));
        }
    }
    assert!(
        visible_prefix > 0 && visible_prefix <= accepted,
        "resident queries must expose a monotonic in-memory prefix"
    );

    let lifecycle = telemetry
        .shutdown()
        .await
        .expect("multi-workspace Telemetry should drain");
    assert_eq!(
        lifecycle.started,
        lifecycle.completed + lifecycle.cancelled + lifecycle.failed
    );
    assert_eq!(lifecycle.active, 0);
    assert_eq!(lifecycle.leaked, 0);
    drop(handle);
    tokio::fs::remove_dir_all(fixture_root)
        .await
        .expect("multi-workspace fixture should release all Turso files");
}

#[tokio::test(flavor = "multi_thread")]
async fn duplicate_budget_failure_identity_is_recorded_once() {
    let _performance = crate::test_support::performance_lock();
    let fixture_root = std::env::temp_dir().join(format!(
        "asp-runtime-otel-failure-dedupe-{}",
        std::process::id()
    ));
    tokio::fs::create_dir_all(&fixture_root)
        .await
        .expect("failure de-duplication fixture root should exist");
    let socket_path = short_unix_socket_path("failure-dedupe-ingress");
    let query_socket_path = short_unix_socket_path("failure-dedupe-query");
    let telemetry = RuntimeServerOpenTelemetry::start(
        fixture_root.join("performance.turso"),
        socket_path,
        query_socket_path.clone(),
    )
    .await
    .expect("failure de-duplication Telemetry should start");
    let handle = telemetry.handle();
    let mut observation = RuntimePerformanceObservation::new(
        "search",
        "graph-turbo-generation-open",
        802_357,
        800_000,
        "budget-exceeded",
    );
    observation.workspace_identity = Some("workspace-failure-dedupe".to_owned());
    observation.failure_reason = Some("runtime-search-io-timeout".to_owned());
    observation.seal_budget_failure_identity();
    assert!(handle.try_record(observation.clone()));
    assert!(handle.try_record(observation));
    tokio::task::yield_now().await;

    let receipt = query_runtime_performance(
        &query_socket_path,
        &RuntimePerformanceQuery::new(
            "workspace-failure-dedupe",
            "search",
            "graph-turbo-generation-open",
        ),
    )
    .await
    .expect("failure de-duplication query should remain resident");
    assert_eq!(receipt.observation_count, 1);
    assert_eq!(receipt.budget_failure_count, 1);

    let lifecycle = telemetry
        .shutdown()
        .await
        .expect("failure de-duplication Telemetry should drain");
    assert_eq!(lifecycle.active, 0);
    assert_eq!(lifecycle.leaked, 0);
    drop(handle);
    tokio::fs::remove_dir_all(fixture_root)
        .await
        .expect("failure de-duplication fixture should release Turso files");
}

#[tokio::test(flavor = "multi_thread")]
async fn failure_adapter_requires_resident_typed_ack_and_is_idempotent() {
    let _performance = crate::test_support::performance_lock();
    let fixture_root = std::env::temp_dir().join(format!(
        "asp-runtime-otel-adapter-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("fixture clock should follow Unix epoch")
            .as_nanos()
    ));
    tokio::fs::create_dir_all(&fixture_root)
        .await
        .expect("adapter fixture root should exist");
    let socket_path = short_unix_socket_path("adapter-ingress");
    let query_socket_path = short_unix_socket_path("adapter-query");
    let telemetry = RuntimeServerOpenTelemetry::start(
        fixture_root.join("performance.turso"),
        socket_path.clone(),
        query_socket_path.clone(),
    )
    .await
    .expect("adapter resident should start");
    let mut observation = RuntimePerformanceObservation::new(
        "query",
        "resident-exact-generation-open",
        801_000,
        800_000,
        "budget-exceeded",
    );
    observation.workspace_identity = Some("workspace-adapter".to_owned());
    observation.failure_reason = Some("fixture-budget-exceeded".to_owned());
    observation.seal_budget_failure_identity();
    let event_identity = observation
        .event_identity
        .clone()
        .expect("budget failure identity should be sealed");
    let recorded = admit_to_runtime(&socket_path, &observation)
        .await
        .expect("first submission must receive a canonical ACK");
    assert_eq!(recorded.state, "recorded");
    assert_eq!(recorded.workspace_identity, "workspace-adapter");
    assert_eq!(recorded.surface, "query");
    assert_eq!(recorded.stage, "resident-exact-generation-open");
    assert_eq!(recorded.event_identity, observation.event_identity);
    let duplicate = admit_to_runtime(&socket_path, &observation)
        .await
        .expect("duplicate submission must receive an idempotent ACK");
    assert_eq!(duplicate.state, "duplicate");
    let receipt = query_runtime_performance(
        &query_socket_path,
        &RuntimePerformanceQuery::new(
            "workspace-adapter",
            "query",
            "resident-exact-generation-open",
        ),
    )
    .await
    .expect("typed ACK must follow resident admission");
    assert_eq!(receipt.observation_count, 1);
    assert_eq!(receipt.budget_failure_count, 1);
    let lifecycle = telemetry.shutdown().await.expect("adapter should drain");
    assert_eq!(lifecycle.active, 0);
    assert_eq!(lifecycle.leaked, 0);
    let durable = TursoOpenTelemetrySpanExporter::open(&fixture_root.join("performance.turso"))
        .await
        .expect("durable event identity reader should open");
    assert_eq!(
        durable
            .budget_failure_count_for_event_identity(&event_identity)
            .await
            .expect("event identity should be directly queryable from Turso"),
        1
    );
    drop(durable);
    tokio::fs::remove_dir_all(fixture_root)
        .await
        .expect("adapter fixture should release Turso files");
}

#[tokio::test(flavor = "multi_thread")]
async fn socket_write_without_resident_ack_is_not_delivery_evidence() {
    let socket_path = short_unix_socket_path("missing-ack");
    let listener =
        tokio::net::UnixListener::bind(&socket_path).expect("missing-ACK fixture should bind");
    let peer = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.expect("fixture should accept");
        let mut bytes = [0_u8; 4_096];
        let _ = tokio::io::AsyncReadExt::read(&mut stream, &mut bytes)
            .await
            .expect("fixture should observe the submitted frame");
    });
    let mut observation = RuntimePerformanceObservation::new(
        "search",
        "graph-turbo-generation-open",
        802_357,
        800_000,
        "budget-exceeded",
    );
    observation.workspace_identity = Some("workspace-missing-ack".to_owned());
    observation.failure_reason = Some("fixture-budget-exceeded".to_owned());
    observation.seal_budget_failure_identity();
    assert!(
        admit_to_runtime(&socket_path, &observation).await.is_err(),
        "a successful socket write without the resident typed ACK must fail closed"
    );
    peer.await.expect("missing-ACK fixture should join");
    tokio::fs::remove_file(socket_path)
        .await
        .expect("missing-ACK socket should be removable");
}

#[test]
fn concurrent_runtime_telemetry_is_nonblocking_and_turso_queryable() {
    let _performance = crate::test_support::performance_lock();
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
        .await
        .expect("resident telemetry should start without opening Turso on the caller");
        let mut wire_observation = RuntimePerformanceObservation::new(
            "workspace-canonical-materialization",
            "payload-accounting",
            801_000,
            800_000,
            "budget-exceeded",
        )
        .with_materialization_metrics(7, 6407, 42, 3_300_000, 5_900_000_000)
        .with_operation_id("fixture-operation");
        wire_observation.workspace_identity = Some("workspace-telemetry-fixture".to_owned());
        wire_observation.process_resident_bytes = Some(512 * 1024 * 1024);
        wire_observation.process_peak_resident_bytes = Some(768 * 1024 * 1024);
        wire_observation.process_memory_budget_bytes = Some(1024 * 1024 * 1024);
        wire_observation.process_memory_budget_status = Some("within-budget".to_owned());
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
        assert_eq!(empty_receipt.observation_count, 0);
        assert_eq!(empty_receipt.p50_micros, None);
        assert_eq!(empty_receipt.p95_micros, None);
        assert_eq!(empty_receipt.p99_micros, None);
        assert!(empty_receipt.latest_attributes_json.is_none());
        let handle = telemetry.handle();
        let dropped_before = handle.dropped_observation_count();
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
                    observation.operation_id =
                        Some(format!("fixture-operation-{worker_index}-{sequence}"));
                    observation.process_resident_bytes = Some(512 * 1024 * 1024);
                    observation.process_peak_resident_bytes = Some(768 * 1024 * 1024);
                    observation.process_memory_budget_bytes = Some(1024 * 1024 * 1024);
                    observation.process_memory_budget_status = Some("within-budget".to_owned());
                    observation.runtime_active_connections = Some(2);
                    observation.runtime_connection_limit = Some(32);
                    observation.runtime_connection_high_watermark = Some(2);
                    observation.runtime_rejected_connections = Some(0);
                    observation.runtime_diagnostic_queue_depth = Some(1);
                    observation.runtime_diagnostic_queue_capacity = Some(256);
                    observation.runtime_dropped_diagnostics = Some(0);
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
        let rejected = (submitted as u64).saturating_sub(accepted);
        let dropped_delta = handle
            .dropped_observation_count()
            .saturating_sub(dropped_before);
        assert!(
            dropped_delta >= rejected,
            "the bounded ingress must account for every rejected batch observation: rejected={rejected} droppedDelta={dropped_delta}"
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
        assert_eq!(
            populated_receipt.observation_count, populated_receipt.budget_failure_count,
            "the fixture submits only budget failures"
        );
        assert_eq!(populated_receipt.p50_micros, Some(801_000));
        assert_eq!(populated_receipt.p95_micros, Some(801_000));
        assert_eq!(populated_receipt.p99_micros, Some(801_000));
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
        let resident_memory = latest_attributes
            .get("asp.process.resident_memory")
            .and_then(serde_json::Value::as_u64)
            .expect("resident Tokio lane should attach its sampled RSS");
        let peak_resident_memory = latest_attributes
            .get("asp.process.peak_resident_memory")
            .and_then(serde_json::Value::as_u64)
            .expect("resident Tokio lane should attach its sampled peak RSS");
        assert!(resident_memory > 0);
        assert!(peak_resident_memory >= resident_memory);
        let event_loop_lag = latest_attributes
            .get("asp.runtime.event_loop.lag")
            .and_then(serde_json::Value::as_u64)
            .expect("resident Tokio lane should publish scheduler lag");
        let event_loop_budget = latest_attributes
            .get("asp.runtime.event_loop.lag.budget")
            .and_then(serde_json::Value::as_u64)
            .expect("resident Tokio lane should publish its scheduler-lag budget");
        assert_eq!(
            latest_attributes
                .get("asp.runtime.event_loop.lag.budget.status")
                .and_then(serde_json::Value::as_str),
            Some(if event_loop_lag > event_loop_budget {
                "budget-exceeded"
            } else {
                "within-budget"
            })
        );
        assert_eq!(event_loop_budget, 10_000);
        assert!(
            latest_attributes
                .get("asp.runtime.worker_threads")
                .and_then(serde_json::Value::as_u64)
                .is_some_and(|workers| workers > 0),
            "resident Telemetry must publish its adaptive Tokio worker count"
        );
        assert!(
            latest_attributes
                .get("asp.runtime.alive_tasks")
                .and_then(serde_json::Value::as_u64)
                .is_some(),
            "resident Telemetry must expose live Tokio task pressure"
        );
        assert!(
            latest_attributes
                .get("asp.runtime.global_queue_depth")
                .and_then(serde_json::Value::as_u64)
                .is_some(),
            "resident Telemetry must expose scheduler queue pressure"
        );
        #[cfg(target_os = "macos")]
        {
            assert!(
                latest_attributes
                    .get("asp.process.disk.read_bytes")
                    .and_then(serde_json::Value::as_u64)
                    .is_some(),
                "macOS Runtime telemetry must expose process disk reads"
            );
            assert!(
                latest_attributes
                    .get("asp.process.disk.write_bytes")
                    .and_then(serde_json::Value::as_u64)
                    .is_some(),
                "macOS Runtime telemetry must expose process disk writes"
            );
            assert!(
                latest_attributes
                    .get("asp.process.page_ins")
                    .and_then(serde_json::Value::as_u64)
                    .is_some(),
                "macOS Runtime telemetry must expose process page-ins"
            );
        }
        let lifecycle = telemetry
            .shutdown()
            .await
            .expect("resident telemetry should drain accepted observations");
        assert_eq!(lifecycle.state, "terminated");
        assert!(
            lifecycle.started >= 7,
            "root, sampler, exporter and every accepted query connection must be owned"
        );
        assert_eq!(
            lifecycle.started,
            lifecycle.completed + lifecycle.cancelled + lifecycle.failed,
            "every admitted Telemetry task must have one terminal outcome"
        );
        assert_eq!(lifecycle.active, 0);
        assert_eq!(lifecycle.leaked, 0);
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
        let pressure_rows = reader
            .runtime_pressure_count_for_workspace("workspace-telemetry-fixture")
            .await
            .expect("typed Runtime Server pressure rows should be queryable from Turso");
        assert_eq!(
            pressure_rows, accepted,
            "every accepted pressure observation must have one typed Turso row"
        );
        drop(reader);
        let _ = tokio::fs::remove_file(socket_path).await;
        let _ = tokio::fs::remove_file(query_socket_path).await;
        let _ = tokio::fs::remove_dir_all(fixture_root).await;
    });
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn stalled_query_connections_are_bounded_and_drain_without_leaks() {
    let fixture_root = std::env::temp_dir().join(format!(
        "asp-runtime-otel-stalled-query-{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("fixture clock")
            .as_nanos()
    ));
    let socket_path = short_unix_socket_path("stalled-query-ingress");
    let query_socket_path = short_unix_socket_path("stalled-query");
    let telemetry = RuntimeServerOpenTelemetry::start(
        fixture_root.join("performance.turso"),
        socket_path.clone(),
        query_socket_path.clone(),
    )
    .await
    .expect("resident telemetry should start");
    let connection_limit =
        agent_semantic_client_db::runtime_server_runtime::runtime_server_connection_limit(
            tokio::runtime::Handle::current().metrics().num_workers(),
        );
    let mut stalled = Vec::with_capacity(connection_limit);
    for _ in 0..connection_limit {
        stalled.push(
            tokio::time::timeout(
                Duration::from_millis(100),
                tokio::net::UnixStream::connect(&query_socket_path),
            )
            .await
            .expect("bounded fixture connect should not stall")
            .expect("fixture should connect to telemetry query socket"),
        );
    }

    let shutdown_started = Instant::now();
    let lifecycle = telemetry
        .shutdown()
        .await
        .expect("stalled query connections must be expired and drained");
    assert!(
        shutdown_started.elapsed() < Duration::from_millis(200),
        "bounded connection expiry must keep shutdown below 200ms: {:?}",
        shutdown_started.elapsed()
    );
    assert_eq!(lifecycle.state, "terminated");
    assert_eq!(lifecycle.active, 0);
    assert_eq!(lifecycle.leaked, 0);
    drop(stalled);
    let _ = tokio::fs::remove_file(socket_path).await;
    let _ = tokio::fs::remove_file(query_socket_path).await;
    let _ = tokio::fs::remove_dir_all(fixture_root).await;
}
