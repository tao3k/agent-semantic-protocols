// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use std::time::Duration;
use std::time::Instant;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;

use agent_semantic_client_db::runtime_server_opentelemetry::RuntimePerformanceObservation;
use agent_semantic_client_db::runtime_server_opentelemetry::RuntimePerformanceQuery;
use agent_semantic_client_db::runtime_server_opentelemetry::RuntimeServerOpenTelemetry;
use agent_semantic_client_db::runtime_server_opentelemetry::query_runtime_performance;

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

mod runtime_search_telemetry_red_contract {
    use agent_semantic_client_db::runtime_server_opentelemetry::RuntimePerformanceObservation;
    use agent_semantic_client_db::runtime_server_opentelemetry::RuntimeSearchTelemetryCollector;
    use agent_semantic_client_db::runtime_server_opentelemetry::RuntimeSearchTelemetryIdentity;
    use agent_semantic_client_db::runtime_server_opentelemetry::RuntimeSearchTelemetryIdentityInput;
    use agent_semantic_client_db::runtime_server_opentelemetry::RuntimeSearchTelemetryTrace;

    const CANONICAL_PHASES: [&str; 10] = [
        "launcher",
        "client-frame-encode",
        "ipc-connect",
        "server-admission-queue",
        "snapshot-resolve",
        "provider-dispatch",
        "parse-index-query",
        "projection-rank",
        "schema-validate-serialize",
        "terminal-egress",
    ];

    fn digest(character: char) -> String {
        format!("blake3-256:{}", character.to_string().repeat(64))
    }

    fn identity(runtime_artifact_digest: &str) -> RuntimeSearchTelemetryIdentity {
        RuntimeSearchTelemetryIdentity::new(RuntimeSearchTelemetryIdentityInput {
            session_id: "session-1".into(),
            request_id: "request-1".into(),
            workspace_identity: "workspace-23cc5ba784c605ae".into(),
            workspace_snapshot_digest: digest('1'),
            source_snapshot_digest: digest('2'),
            source_generation_digest: digest('3'),
            source_index_digest: digest('4'),
            runtime_artifact_digest: runtime_artifact_digest.into(),
            runtime_bundle_digest: digest('5'),
            execution_publication_digest: digest('6'),
            provider_catalog_digest: digest('7'),
            language_ids: vec!["rust".into()],
            provider_ids: vec!["asp-rust".into()],
        })
        .expect("fixture identity must be canonical")
    }

    #[test]
    fn telemetry_identity_retains_the_complete_execution_product() {
        let identity = identity(&digest('a'));
        assert_eq!(identity.session_id, "session-1");
        assert_eq!(identity.source_index_digest, digest('4'));
        assert_eq!(identity.runtime_bundle_digest, digest('5'));
        assert_eq!(identity.execution_publication_digest, digest('6'));
    }

    #[test]
    fn telemetry_identity_rejects_noncanonical_language_and_provider_sets() {
        let error = RuntimeSearchTelemetryIdentity::new(RuntimeSearchTelemetryIdentityInput {
            session_id: "session-1".into(),
            request_id: "request-1".into(),
            workspace_identity: "workspace-23cc5ba784c605ae".into(),
            workspace_snapshot_digest: digest('1'),
            source_snapshot_digest: digest('2'),
            source_generation_digest: digest('3'),
            source_index_digest: digest('4'),
            runtime_artifact_digest: digest('a'),
            runtime_bundle_digest: digest('5'),
            execution_publication_digest: digest('6'),
            provider_catalog_digest: digest('7'),
            language_ids: vec!["rust".into(), "python".into(), "rust".into()],
            provider_ids: vec!["asp-rust".into()],
        })
        .expect_err("identity sets must be sorted and unique");
        assert_eq!(
            error.reason_kind(),
            "runtime-search-telemetry-identity-mismatch"
        );
    }

    #[test]
    fn search_phase_observation_retains_the_complete_execution_product() {
        let identity = identity(&digest('a'));
        let observation = RuntimePerformanceObservation::for_search_phase(
            &identity,
            "snapshot-resolve",
            41,
            1_000,
            "within-budget",
        )
        .expect("a canonical search phase must produce a typed observation");

        assert_eq!(observation.session_id.as_deref(), Some("session-1"));
        assert_eq!(observation.request_id.as_deref(), Some("request-1"));
        assert_eq!(
            observation.workspace_snapshot_digest.as_deref(),
            Some(digest('1').as_str())
        );
        assert_eq!(
            observation.source_snapshot_digest.as_deref(),
            Some(digest('2').as_str())
        );
        assert_eq!(
            observation.source_index_digest.as_deref(),
            Some(digest('4').as_str())
        );
        assert_eq!(
            observation.runtime_bundle_digest.as_deref(),
            Some(digest('5').as_str())
        );
        assert_eq!(
            observation.execution_publication_digest.as_deref(),
            Some(digest('6').as_str())
        );
        assert_eq!(
            observation.provider_catalog_digest.as_deref(),
            Some(digest('7').as_str())
        );
        assert_eq!(observation.provider_id.as_deref(), Some("asp-rust"));
        assert_eq!(observation.language_id.as_deref(), Some("rust"));
        assert_eq!(observation.language_ids, Some(vec!["rust".into()]));
        assert_eq!(observation.provider_ids, Some(vec!["asp-rust".into()]));
        assert_eq!(observation.stage, "snapshot-resolve");
    }

    #[test]
    fn performance_observation_schema_covers_every_serialized_field() {
        let identity = identity(&digest('a'));
        let observation = RuntimePerformanceObservation::for_search_phase(
            &identity,
            "snapshot-resolve",
            41,
            1_000,
            "within-budget",
        )
        .expect("canonical observation");
        let serialized =
            serde_json::to_value(observation).expect("performance observation must serialize");
        let schema: serde_json::Value = serde_json::from_str(include_str!(
            "../../../../schemas/runtime-server-performance-observation.v1.schema.json"
        ))
        .expect("performance observation schema must decode");
        let schema_properties = schema["properties"].as_object().expect("schema properties");

        for field in serialized
            .as_object()
            .expect("serialized observation")
            .keys()
        {
            assert!(
                schema_properties.contains_key(field),
                "serialized observation field is absent from the closed schema: {field}"
            );
        }
    }

    #[test]
    fn canonical_search_phases_emit_exactly_one_terminal() {
        let identity = identity(&digest('a'));
        let mut collector = RuntimeSearchTelemetryCollector::new(identity.clone(), 4_096)
            .expect("bounded collector must initialize");

        for (sequence, phase) in CANONICAL_PHASES.iter().enumerate() {
            collector
                .record_phase(&identity, phase, sequence as u64)
                .expect("canonical phase must be admitted");
        }
        collector
            .record_terminal(&identity, "accepted", 10)
            .expect("first terminal must be admitted");

        let duplicate = collector
            .record_terminal(&identity, "accepted", 11)
            .expect_err("a second terminal must be rejected");
        assert_eq!(duplicate.reason_kind(), "runtime-search-terminal-duplicate");

        let artifact = collector
            .to_artifact()
            .expect("complete telemetry must produce an artifact");
        assert_eq!(artifact.phase_names(), CANONICAL_PHASES);
        assert_eq!(artifact.terminal_count(), 1);
    }

    #[test]
    fn observation_rejects_artifact_refresh_identity_mismatch() {
        let admitted = identity(&digest('a'));
        let refreshed = identity(&digest('b'));
        let mut collector = RuntimeSearchTelemetryCollector::new(admitted.clone(), 4_096)
            .expect("bounded collector must initialize");

        collector
            .record_phase(&admitted, CANONICAL_PHASES[0], 0)
            .expect("admitted identity must be recorded");
        let mismatch = collector
            .record_phase(&refreshed, CANONICAL_PHASES[1], 1)
            .expect_err("artifact refresh must not mutate an admitted request identity");
        assert_eq!(
            mismatch.reason_kind(),
            "runtime-search-telemetry-identity-mismatch"
        );
    }

    #[test]
    fn metric_labels_are_bounded_and_reject_request_cardinality() {
        let identity = identity(&digest('a'));
        let mut collector = RuntimeSearchTelemetryCollector::new(identity, 4_096)
            .expect("bounded collector must initialize");

        collector
            .record_metric_label("language_id", "rust")
            .expect("bounded language label must be admitted");
        let high_cardinality = collector
            .record_metric_label("request_id", "request-1")
            .expect_err("request identity must never become a metric label");
        assert_eq!(
            high_cardinality.reason_kind(),
            "runtime-search-metric-label-cardinality"
        );
    }

    #[test]
    fn bounded_collector_reserves_terminal_and_rejects_capacity_exhaustion() {
        let identity = identity(&digest('a'));
        let mut collector = RuntimeSearchTelemetryCollector::new(identity.clone(), 2)
            .expect("bounded collector must initialize");

        collector
            .record_phase(&identity, CANONICAL_PHASES[0], 0)
            .expect("one phase must fit while reserving terminal capacity");
        let exhausted = collector
            .record_phase(&identity, CANONICAL_PHASES[1], 1)
            .expect_err("non-terminal events must not consume reserved terminal capacity");
        assert_eq!(
            exhausted.reason_kind(),
            "runtime-search-telemetry-capacity-exhausted"
        );
        collector
            .record_terminal(&identity, "failed", 2)
            .expect("terminal must remain observable under pressure");
    }

    #[test]
    fn request_trace_enforces_phase_authority_and_transport_owned_terminal() {
        let identity = identity(&digest('a'));
        let trace = RuntimeSearchTelemetryTrace::new(identity.clone());

        for phase in &CANONICAL_PHASES[..3] {
            trace
                .record_client_phase(phase, 1, 1_000)
                .expect("client-owned pre-admission phase");
        }
        assert_eq!(
            trace
                .record_search_execution(1, 1_000)
                .expect_err("SearchExecution cannot skip the admitted predecessors")
                .reason_kind(),
            "runtime-search-telemetry-phase-order"
        );
        trace
            .record_server_admission_queue(1, 1_000)
            .expect("admission boundary");
        trace
            .record_snapshot_resolve(1, 1_000)
            .expect("snapshot boundary");
        trace
            .record_provider_dispatch(1, 1_000)
            .expect("provider dispatch boundary");
        trace
            .record_search_execution(1, 1_000)
            .expect("SearchExecution boundary");
        trace
            .record_search_projection(1, 1_000)
            .expect("SearchProjection boundary");
        trace
            .record_response_serialized(1, 1_000)
            .expect("response serialization boundary");
        trace
            .record_transport_terminal_egress("ready", 1, 1_000)
            .expect("transport writer owns exactly one terminal egress");
        assert_eq!(trace.observations().len(), 10);
        assert_eq!(trace.terminal_count(), 1);
        assert_eq!(
            trace
                .record_transport_terminal_egress("ready", 1, 1_000)
                .expect_err("duplicate terminal")
                .reason_kind(),
            "runtime-search-terminal-duplicate"
        );
    }

    #[test]
    fn incomplete_request_trace_preserves_one_transport_terminal_without_fabricated_phases() {
        let identity = identity(&digest('a'));
        let trace = RuntimeSearchTelemetryTrace::new(identity);

        trace
            .record_client_phase("launcher", 1, 1_000)
            .expect("the observed client phase is retained");
        trace
            .record_transport_terminal_egress("transport-closed", 2, 1_000)
            .expect("a failed transport still owns one terminal observation");

        let observations = trace.observations();
        assert_eq!(observations.len(), 2);
        assert_eq!(observations[0].stage, "launcher");
        assert_eq!(observations[1].stage, "terminal-egress");
        assert_eq!(
            observations[1].failure_reason.as_deref(),
            Some("transport-closed")
        );
        assert_eq!(trace.terminal_count(), 1);
        assert_eq!(
            trace
                .record_transport_terminal_egress("transport-closed", 3, 1_000)
                .expect_err("the transport cannot emit a second terminal")
                .reason_kind(),
            "runtime-search-terminal-duplicate"
        );
    }
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

#[path = "runtime_server_opentelemetry_delivery.rs"]
mod delivery_tests;
