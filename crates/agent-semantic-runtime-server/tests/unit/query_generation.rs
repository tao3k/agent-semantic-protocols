// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use super::RuntimeProjectWorkspaceKey;
use super::RuntimeSearchGenerationBuildResourceInput;
use crate::RuntimeQueryGenerationAuthority;
use crate::RuntimeQueryGenerationState;
use crate::query_generation_calibration::RuntimeSearchCalibrationDecision;
use crate::query_generation_calibration::RuntimeSearchCalibrationStore;
use crate::query_generation_calibration::cached_calibration_decisions;
use crate::query_generation_calibration::runtime_search_calibration_key;
use crate::query_generation_calibration::select_runtime_search_build_resources;
use crate::query_generation_calibration::select_single_segment_bulk;
use crate::query_generation_calibration::upsert_runtime_search_calibration_decision;
use crate::query_generation_calibration::workload_bucket;
use crate::runtime_query_generation::RuntimeQueryTerminalState;
use crate::runtime_query_generation::RuntimeSearchTerminalState;

#[path = "query_generation_materialization_retention.rs"]
mod materialization_retention;

fn key(project_id: &str, workspace_id: &str) -> RuntimeProjectWorkspaceKey {
    RuntimeProjectWorkspaceKey::new(
        agent_semantic_client_protocol::ClientProjectId::new(project_id).expect("test ProjectId"),
        agent_semantic_client_protocol::ClientWorkspaceIdentity::new(workspace_id)
            .expect("test WorkspaceId"),
    )
}

fn authority() -> RuntimeQueryGenerationAuthority {
    RuntimeQueryGenerationAuthority::new_in_task_scope(
        agent_semantic_workspace_scheduler::RuntimeServerTaskScope::new(
            "runtime-query-generation-test",
        ),
        agent_semantic_workspace_scheduler::RuntimeServerResourceSupervisor::new(
            4,
            256 * 1024 * 1024,
        ),
    )
    .expect("test query generation authority")
}

#[test]
fn build_resources_reduce_workers_under_memory_pressure() {
    let throughput = std::collections::BTreeMap::from([(1, 100), (2, 200)]);
    let receipt = select_runtime_search_build_resources(
        RuntimeSearchGenerationBuildResourceInput {
            effective_cpu: 16,
            server_worker_ceiling: 8,
            process_memory_budget_bytes: 30,
            blocking_lane_pressure: 0,
            lexical_bytes: 10,
            owner_count: 4_096,
            changed_owner_count: 4_096,
        },
        10,
        &throughput,
        "parallel-segments",
    )
    .expect("low-memory resource envelope");
    assert_eq!(receipt.chosen_workers, 3);
    assert_eq!(receipt.memory_budget_bytes, 30);
}

#[test]
fn build_resources_scale_up_while_preserving_request_headroom() {
    let throughput = std::collections::BTreeMap::from([(1, 100), (2, 200), (4, 350)]);
    let receipt = select_runtime_search_build_resources(
        RuntimeSearchGenerationBuildResourceInput {
            effective_cpu: 16,
            server_worker_ceiling: 8,
            process_memory_budget_bytes: 1_000,
            blocking_lane_pressure: 0,
            lexical_bytes: 1_000,
            owner_count: 4_096,
            changed_owner_count: 4_096,
        },
        10,
        &throughput,
        "parallel-segments",
    )
    .expect("high-capacity resource envelope");
    assert_eq!(receipt.chosen_workers, 8);
    assert!(receipt.chosen_workers < receipt.effective_cpu);
}

#[test]
fn unseen_full_generation_uses_a_machine_relative_lower_bracket_once() {
    let receipt = select_runtime_search_build_resources(
        RuntimeSearchGenerationBuildResourceInput {
            effective_cpu: 16,
            server_worker_ceiling: 8,
            process_memory_budget_bytes: 1_000,
            blocking_lane_pressure: 2,
            lexical_bytes: 1 << 20,
            owner_count: 4_096,
            changed_owner_count: 4_096,
        },
        10,
        &std::collections::BTreeMap::new(),
        "parallel-segments",
    )
    .expect("unseen workload resource envelope");
    assert_eq!(receipt.chosen_workers, 3);
    assert_eq!(receipt.memory_budget_bytes, 30);
    assert_eq!(receipt.observed_owners_per_second, 0);
}

#[test]
fn delta_resources_stop_at_measured_marginal_throughput_regression() {
    let throughput = std::collections::BTreeMap::from([(1, 100), (2, 200), (4, 190)]);
    let receipt = select_runtime_search_build_resources(
        RuntimeSearchGenerationBuildResourceInput {
            effective_cpu: 16,
            server_worker_ceiling: 8,
            process_memory_budget_bytes: 1_000,
            blocking_lane_pressure: 0,
            lexical_bytes: 1_000,
            owner_count: 4_096,
            changed_owner_count: 4_096,
        },
        10,
        &throughput,
        "parallel-segments",
    )
    .expect("historical throughput resource envelope");
    assert_eq!(receipt.chosen_workers, 2);
    assert_eq!(receipt.observed_owners_per_second, 200);

    let delta = select_runtime_search_build_resources(
        RuntimeSearchGenerationBuildResourceInput {
            effective_cpu: 16,
            server_worker_ceiling: 8,
            process_memory_budget_bytes: 1_000,
            blocking_lane_pressure: 0,
            lexical_bytes: 10,
            owner_count: 4_096,
            changed_owner_count: 1,
        },
        10,
        &std::collections::BTreeMap::new(),
        "parallel-segments",
    )
    .expect("small delta resource envelope");
    assert_eq!(delta.chosen_workers, 1);
    assert!(delta.changed_owner_ratio < 0.001);
}

#[test]
fn workload_history_is_partitioned_by_strategy_size_and_change_ratio() {
    let bulk = workload_bucket(1 << 20, 1_024, 1_024, true);
    let delta = workload_bucket(1 << 20, 1_024, 1, false);
    let larger = workload_bucket(1 << 24, 16_384, 16_384, true);
    assert_ne!(bulk, delta);
    assert_ne!(bulk, larger);

    let mut history = std::collections::BTreeMap::new();
    history.insert(bulk, std::collections::BTreeMap::from([(1, 10), (2, 20)]));
    assert!(!history.contains_key(&delta));
    assert!(!history.contains_key(&larger));
}

#[test]
fn first_multi_owner_generation_uses_parallel_seed_and_history_chooses_strategy() {
    let empty = std::collections::BTreeMap::new();
    assert!(!select_single_segment_bulk(4_096, &empty, &empty));
    assert!(select_single_segment_bulk(1, &empty, &empty));

    let bulk = std::collections::BTreeMap::from([(1, 400)]);
    let parallel = std::collections::BTreeMap::from([(4, 1_200)]);
    assert!(!select_single_segment_bulk(4_096, &bulk, &parallel));
    let faster_bulk = std::collections::BTreeMap::from([(1, 1_300)]);
    assert!(select_single_segment_bulk(4_096, &faster_bulk, &parallel));
}

#[tokio::test]
async fn authority_starts_empty_and_can_clear_all() {
    let authority = authority();
    let receiver = authority.subscribe();
    assert!(receiver.borrow().is_empty());
    authority.clear_all();
    assert!(receiver.borrow().is_empty());
}

#[tokio::test]
async fn cold_build_admission_is_scoped_to_an_absent_workspace() {
    let authority = authority();
    let rust_key = key("project-live-corpus", "workspace-live-corpus-rust");
    authority
        .require_workspace_absent(&rust_key)
        .expect("fresh benchmark workspace");
    authority.publish_failed(
        rust_key.clone(),
        1,
        format!("blake3-256:{}", "a".repeat(64)),
        "fixture failure",
    );
    let error = authority
        .require_workspace_absent(&rust_key)
        .expect_err("published benchmark workspace must not be reused as cold-build");
    assert!(error.contains("cold-build-workspace-already-published"));
    authority
        .require_workspace_absent(&key("project-live-corpus", "workspace-live-corpus-python"))
        .expect("another language workspace is unaffected");
    authority
        .require_workspace_absent(&key("project-other", "workspace-live-corpus-rust"))
        .expect("the same WorkspaceId in another ProjectId is isolated");
}

#[tokio::test]
async fn failed_publication_is_bound_to_the_expected_generation_digest() {
    let authority = authority();
    let receiver = authority.subscribe();
    let workspace_key = key("project-test", "workspace-test");

    authority.publish_failed(
        workspace_key.clone(),
        0,
        "blake3-256:expected",
        "resident open failed",
    );

    let observed = receiver.borrow();
    let Some(RuntimeQueryGenerationState::Failed {
        expected_generation_digest,
        reason,
    }) = observed.get(&workspace_key)
    else {
        panic!("failed publication must retain generation identity");
    };
    assert_eq!(expected_generation_digest.as_ref(), "blake3-256:expected");
    assert_eq!(reason.as_ref(), "resident open failed");
}

#[tokio::test]
async fn failed_open_publishes_a_typed_workspace_state() {
    let authority = authority();
    let receiver = authority.subscribe();
    let workspace_key = key("project-test", "workspace-test");
    let missing = std::path::Path::new("/definitely-missing-asp-generation/pointer");

    let result = authority
        .ensure_ready(
            &workspace_key,
            missing,
            std::path::Path::new("/definitely-missing-asp-generation/project"),
            "blake3-256:expected",
        )
        .await;
    let Err(error) = result else {
        panic!("missing generation must fail");
    };

    assert!(!error.is_empty());
    assert!(matches!(
        receiver.borrow().get(&workspace_key),
        Some(RuntimeQueryGenerationState::Failed { .. })
    ));
}

#[test]
fn calibration_cache_ignores_stale_engine_and_machine_identity() {
    let bulk = workload_bucket(1 << 20, 4_096, 4_096, true);
    let parallel = workload_bucket(1 << 20, 4_096, 4_096, false);
    let mut store = RuntimeSearchCalibrationStore::default();
    upsert_runtime_search_calibration_decision(
        &mut store,
        runtime_search_calibration_key("engine-a", 8, 120, parallel),
        RuntimeSearchCalibrationDecision {
            strategy: "parallel-segments".to_owned(),
            workers: 4,
            memory_budget_bytes: 60,
            observed_owners_per_second: 846,
            sample_identity: "blake3-256:sample".to_owned(),
        },
    );
    assert!(cached_calibration_decisions(&store, "engine-b", 8, 120, bulk, parallel).is_empty());
    assert!(cached_calibration_decisions(&store, "engine-a", 4, 60, bulk, parallel).is_empty());
    assert_eq!(
        cached_calibration_decisions(&store, "engine-a", 8, 120, bulk, parallel)[0].workers,
        4
    );
}

#[test]
fn production_observations_accumulate_across_resource_brackets() {
    let workload = workload_bucket(1 << 20, 4_096, 4_096, false);
    let key = runtime_search_calibration_key("engine-a", 8, 120, workload);
    let mut store = RuntimeSearchCalibrationStore::default();
    for (workers, throughput) in [(2, 400), (4, 900)] {
        upsert_runtime_search_calibration_decision(
            &mut store,
            key.clone(),
            RuntimeSearchCalibrationDecision {
                strategy: "parallel-segments".to_owned(),
                workers,
                memory_budget_bytes: workers * 15,
                observed_owners_per_second: throughput,
                sample_identity: format!("blake3-256:generation-{workers}"),
            },
        );
    }
    let observed = cached_calibration_decisions(&store, "engine-a", 8, 120, workload, workload);
    assert_eq!(observed.len(), 2);
    assert_eq!(observed[0].workers, 2);
    assert_eq!(observed[1].workers, 4);
}

fn test_generation(digest: &str) -> std::sync::Arc<super::RuntimeQueryGeneration> {
    std::sync::Arc::new(super::RuntimeQueryGeneration {
        generation_digest: digest.to_owned(),
        generation_token: std::sync::atomic::AtomicU64::new(0),
        resident: None,
        resource_supervisor:
            agent_semantic_workspace_scheduler::RuntimeServerResourceSupervisor::new(
                2,
                16 * 1024 * 1024,
            ),
        task_scope: agent_semantic_workspace_scheduler::RuntimeServerTaskScope::new(
            "query-generation-test",
        ),
        execution_publication: None,
        project_topology_attachment: std::sync::Mutex::new(None),
        project_topology_build_lock: tokio::sync::Mutex::new(()),
        resident_syntax_scope_evidence: std::sync::Mutex::new(None),
        lexical_attachment_completion: tokio::sync::watch::channel(false).0,
        build_resource_receipt: std::sync::OnceLock::new(),
        search_materializations: std::sync::Arc::new(std::sync::Mutex::new(
            std::collections::HashMap::new(),
        )),
        query_materializations: std::sync::Arc::new(std::sync::Mutex::new(
            std::collections::HashMap::new(),
        )),
        materialization_tasks: std::sync::Arc::new(std::sync::Mutex::new(
            tokio::task::JoinSet::new(),
        )),
    })
}

#[tokio::test]
async fn first_call_single_flight_terminal_is_scenario_measured() {
    use asp_rust_project_harness_policy::{
        AspRustProjectHarnessScenarioObservation, FIRST_CALL_SINGLE_FLIGHT_TERMINAL_SCENARIO_ID,
        asp_search_scenario_package, measure_asp_rust_scenario,
        render_asp_rust_scenario_benchmark_toml,
    };

    const CALLERS: usize = 32;
    async fn exercise(callers: usize) -> std::time::Duration {
        let generation = test_generation("first-call-single-flight");
        let search_key = "blake3-256:first-search-call".to_owned();
        let query_key = "source\0blake3-256:first-query-call".to_owned();
        let started = std::time::Instant::now();
        assert!(
            generation
                .begin_search_materialization(search_key.clone())
                .unwrap()
        );
        assert!(
            generation
                .begin_query_materialization(query_key.clone())
                .unwrap()
        );
        for _ in 1..callers {
            assert!(
                !generation
                    .begin_search_materialization(search_key.clone())
                    .unwrap()
            );
            assert!(
                !generation
                    .begin_query_materialization(query_key.clone())
                    .unwrap()
            );
        }
        let mut search_waiters = tokio::task::JoinSet::new();
        let mut query_waiters = tokio::task::JoinSet::new();
        for _ in 0..callers {
            let waiting_generation = std::sync::Arc::clone(&generation);
            let waiting_key = search_key.clone();
            search_waiters.spawn(async move {
                waiting_generation
                    .await_search_materialization(&waiting_key)
                    .await
            });
            let waiting_generation = std::sync::Arc::clone(&generation);
            let waiting_key = query_key.clone();
            query_waiters.spawn(async move {
                waiting_generation
                    .await_query_materialization(&waiting_key)
                    .await
            });
        }
        let publishing_generation = std::sync::Arc::clone(&generation);
        generation
            .spawn_materialization("first-search-call", async move {
                tokio::task::yield_now().await;
                publishing_generation
                    .publish_search_materialization(
                        search_key,
                        Ok(serde_json::json!({"terminal": {"state": "ready"}})),
                    )
                    .expect("publish Search terminal");
            })
            .expect("spawn generation-owned Search computation");
        let publishing_generation = std::sync::Arc::clone(&generation);
        generation
            .spawn_materialization("first-query-call", async move {
                tokio::task::yield_now().await;
                publishing_generation
                    .publish_query_materialization(
                        query_key,
                        Ok(serde_json::json!({"terminal": {"state": "ready"}})),
                    )
                    .expect("publish Query terminal");
            })
            .expect("spawn generation-owned Query computation");
        let mut search_terminal_count = 0;
        while let Some(joined) = search_waiters.join_next().await {
            assert!(matches!(
                joined.unwrap().unwrap(),
                RuntimeSearchTerminalState::Ready(value)
                    if value["terminal"]["state"] == "ready"
            ));
            search_terminal_count += 1;
        }
        let mut query_terminal_count = 0;
        while let Some(joined) = query_waiters.join_next().await {
            assert!(matches!(
                joined.unwrap().unwrap(),
                RuntimeQueryTerminalState::Ready(value)
                    if value["terminal"]["state"] == "ready"
            ));
            query_terminal_count += 1;
        }
        assert_eq!(search_terminal_count, callers);
        assert_eq!(query_terminal_count, callers);
        loop {
            let receipt = generation.task_scope.receipt(0);
            if receipt.active == 0 {
                assert_eq!(receipt.started, 2);
                assert_eq!(receipt.completed, 2);
                assert_eq!(receipt.cancelled, 0);
                break;
            }
            tokio::task::yield_now().await;
        }
        started.elapsed()
    }

    // The first awaiter models the claiming request. It receives the retained
    // terminal without a second invocation or a public Building response.
    let first_elapsed = exercise(CALLERS).await;
    let scenario = asp_search_scenario_package()
        .scenarios
        .into_iter()
        .find(|scenario| scenario.name == FIRST_CALL_SINGLE_FLIGHT_TERMINAL_SCENARIO_ID)
        .expect("first-call single-flight Scenario");
    let measurement = tokio::task::spawn_blocking(move || {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("Scenario runtime");
        measure_asp_rust_scenario(&scenario, || {
            let elapsed = runtime.block_on(exercise(CALLERS));
            AspRustProjectHarnessScenarioObservation::default()
                .with_timing("retained_terminal_fanout", elapsed)
                .with_metric("search_computation_claim_count", 1)
                .with_metric("query_computation_claim_count", 1)
                .with_metric("terminal_waiter_count", (CALLERS * 2) as u64)
                .with_metric("generation_owned_task_count", 2)
                .with_metric("caller_retry_count", 0)
                .with_metric("public_building_terminal_count", 0)
        })
        .map(|measurement| (scenario, measurement))
    })
    .await
    .expect("join Scenario measurement")
    .expect("measure first-call single-flight Scenario");
    let rendered = render_asp_rust_scenario_benchmark_toml(&measurement.0, &measurement.1)
        .expect("render first-call single-flight benchmark");
    assert!(rendered.contains("[metrics.public_building_terminal_count]"));
    assert!(rendered.contains("observed = 0"));
    eprintln!(
        "first-call retained terminal elapsedMicros={}\n{rendered}",
        first_elapsed.as_micros()
    );
}

#[test]
fn resident_syntax_scope_cache_requires_exact_plan_and_owner_set() {
    let generation = test_generation("syntax-scope-cache-generation");
    let scope = std::collections::BTreeSet::from(["src/lib.rs".to_owned()]);
    let evidence = std::sync::Arc::new(Vec::new());
    generation
        .publish_resident_syntax_scope_evidence(
            "plan-a".to_owned(),
            scope.clone(),
            std::sync::Arc::clone(&evidence),
        )
        .expect("publish exact structural evidence");
    assert!(
        generation
            .resident_syntax_scope_evidence("plan-a", &scope)
            .unwrap()
            .is_some()
    );
    assert!(
        generation
            .resident_syntax_scope_evidence("plan-b", &scope)
            .unwrap()
            .is_none()
    );
    assert!(
        generation
            .resident_syntax_scope_evidence(
                "plan-a",
                &std::collections::BTreeSet::from(["src/other.rs".to_owned()]),
            )
            .unwrap()
            .is_none()
    );
}

#[tokio::test]
async fn lexical_completion_does_not_wait_for_request_graph() {
    let generation = test_generation("lexical-ready-request-graph-pending");
    let (finish_graph, graph_pending) = tokio::sync::oneshot::channel::<()>();
    let graph_task = tokio::spawn(async move {
        let _ = graph_pending.await;
    });

    generation.publish_lexical_attachment_terminal();
    tokio::time::timeout(
        std::time::Duration::from_millis(50),
        generation.await_lexical_attachment(),
    )
    .await
    .expect("lexical waiter must not join request Graph")
    .expect("lexical terminal is retained");
    assert!(!graph_task.is_finished());

    finish_graph.send(()).expect("finish request Graph fixture");
    graph_task.await.expect("request Graph fixture joins");
}

#[tokio::test]
async fn search_waiters_share_one_completion_and_late_subscribers_observe_it() {
    let generation = test_generation("generation-wait");
    assert!(
        generation
            .begin_search_materialization("key".into())
            .unwrap()
    );
    let mut waiters = tokio::task::JoinSet::new();
    for _ in 0..32 {
        assert!(
            !generation
                .begin_search_materialization("key".into())
                .unwrap()
        );
        let generation = std::sync::Arc::clone(&generation);
        waiters.spawn(async move { generation.await_search_materialization("key").await });
    }
    tokio::task::yield_now().await;
    generation
        .publish_search_materialization("key".into(), Ok(serde_json::json!({"hits": 1})))
        .unwrap();
    while let Some(joined) = waiters.join_next().await {
        assert!(
            matches!(joined.unwrap().unwrap(), RuntimeSearchTerminalState::Ready(value) if value["hits"] == 1)
        );
    }
    assert!(matches!(
        generation
            .await_search_materialization("key")
            .await
            .unwrap(),
        RuntimeSearchTerminalState::Ready(_)
    ));
}
