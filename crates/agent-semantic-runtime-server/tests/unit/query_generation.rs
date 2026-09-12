// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use super::RuntimeProjectWorkspaceKey;
use super::RuntimeSearchDerivedAttachmentIdentity;
use super::RuntimeSearchDerivedAttachmentKind;
use super::RuntimeSearchDerivedAttachmentState;
use super::RuntimeSearchGenerationBuildJob;
use super::RuntimeSearchGenerationBuildOperation;
use super::RuntimeSearchGenerationBuildResourceInput;
use super::RuntimeSearchGenerationBuilder;
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
use crate::runtime_query_generation::RuntimeQueryMaterializationState;
use crate::runtime_query_generation::RuntimeSearchMaterializationState;

fn key(project_id: &str, workspace_id: &str) -> RuntimeProjectWorkspaceKey {
    RuntimeProjectWorkspaceKey::new(
        agent_semantic_client_protocol::ClientProjectId::new(project_id).expect("test ProjectId"),
        agent_semantic_client_protocol::ClientWorkspaceIdentity::new(workspace_id)
            .expect("test WorkspaceId"),
    )
}

fn authority() -> RuntimeQueryGenerationAuthority {
    RuntimeQueryGenerationAuthority::new_in_task_scope(
        agent_semantic_client_db::runtime_server_runtime::RuntimeServerTaskScope::new(
            "runtime-query-generation-test",
        ),
        agent_semantic_client_db::runtime_server_runtime::RuntimeServerResourceSupervisor::new(
            4,
            256 * 1024 * 1024,
        ),
    )
    .expect("test query generation authority")
}

fn attachment_identity(
    attachment: RuntimeSearchDerivedAttachmentKind,
) -> RuntimeSearchDerivedAttachmentIdentity {
    RuntimeSearchDerivedAttachmentIdentity {
        project_id: "project-test".to_owned(),
        workspace_id: "workspace-test".to_owned(),
        generation_token: 1,
        content_generation_digest: format!("blake3-256:{}", "a".repeat(64)),
        attachment,
    }
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
        execution_publication: None,
        project_topology_attachment: std::sync::OnceLock::new(),
        project_topology_completion: tokio::sync::watch::channel(false).0,
        lexical_attachment_completion: tokio::sync::watch::channel(false).0,
        build_resource_receipt: std::sync::OnceLock::new(),
        search_materializations: std::sync::Mutex::new(std::collections::HashMap::new()),
        query_materializations: std::sync::Mutex::new(std::collections::HashMap::new()),
    })
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
    use crate::runtime_query_generation::RuntimeSearchMaterializationState;
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
            matches!(joined.unwrap().unwrap(), RuntimeSearchMaterializationState::Ready(value) if value["hits"] == 1)
        );
    }
    assert!(matches!(
        generation
            .await_search_materialization("key")
            .await
            .unwrap(),
        RuntimeSearchMaterializationState::Ready(_)
    ));
}

#[tokio::test]
async fn cancelled_query_waiter_does_not_cancel_shared_completion() {
    use crate::runtime_query_generation::RuntimeQueryMaterializationState;
    let generation = test_generation("generation-cancel");
    assert!(
        generation
            .begin_query_materialization("key".into())
            .unwrap()
    );
    let waiting_generation = std::sync::Arc::clone(&generation);
    let waiter =
        tokio::spawn(async move { waiting_generation.await_query_materialization("key").await });
    tokio::task::yield_now().await;
    waiter.abort();
    assert!(matches!(waiter.await, Err(error) if error.is_cancelled()));
    generation
        .publish_query_materialization("key".into(), Ok(serde_json::json!({"source": "exact"})))
        .unwrap();
    assert!(
        matches!(generation.await_query_materialization("key").await.unwrap(), RuntimeQueryMaterializationState::Ready(value) if value["source"] == "exact")
    );
}

#[tokio::test]
async fn terminal_failure_wakes_waiters_without_claiming_an_empty_result() {
    use crate::runtime_query_generation::RuntimeSearchMaterializationState;
    let generation = test_generation("generation-error");
    assert!(
        generation
            .begin_search_materialization("key".into())
            .unwrap()
    );
    let waiting_generation = std::sync::Arc::clone(&generation);
    let waiter =
        tokio::spawn(async move { waiting_generation.await_search_materialization("key").await });
    tokio::task::yield_now().await;
    generation
        .publish_search_materialization(
            "key".into(),
            Err(agent_semantic_client_server::AspClientDispatchError {
                reason_kind: "budget-exhausted".into(),
                message: "fixture".into(),
                details: None,
            }),
        )
        .unwrap();
    assert!(
        matches!(waiter.await.unwrap().unwrap(), RuntimeSearchMaterializationState::Failed(error) if error.reason_kind == "budget-exhausted")
    );
}

#[test]
fn search_materialization_claims_once_and_publishes_one_terminal() {
    let generation = test_generation("blake3-256:generation-a");
    let key = "blake3-256:search-a".to_owned();

    assert!(
        generation
            .begin_search_materialization(key.clone())
            .unwrap()
    );
    assert!(
        !generation
            .begin_search_materialization(key.clone())
            .unwrap()
    );
    assert!(matches!(
        generation.search_materialization(&key).unwrap(),
        Some(RuntimeSearchMaterializationState::Building(_))
    ));

    let value = serde_json::json!({"result": "resident"});
    generation
        .publish_search_materialization(key.clone(), Ok(value.clone()))
        .unwrap();
    let Some(RuntimeSearchMaterializationState::Ready(published)) =
        generation.search_materialization(&key).unwrap()
    else {
        panic!("claimed materialization must publish Ready")
    };
    assert_eq!(published.as_ref(), &value);
    assert!(
        generation
            .publish_search_materialization(key, Ok(value))
            .is_err()
    );
}

#[tokio::test]
async fn result_wait_deadline_preserves_late_completion_and_notification() {
    use crate::runtime_query_generation::RuntimeSearchMaterializationState;
    let generation = test_generation("generation-deadline");
    assert!(
        generation
            .begin_search_materialization("key".into())
            .unwrap()
    );
    let Some(RuntimeSearchMaterializationState::Building(completion)) =
        generation.search_materialization("key").unwrap()
    else {
        panic!("expected claimed materialization");
    };
    assert!(
        tokio::time::timeout(
            std::time::Duration::from_millis(1),
            generation.await_search_materialization("key")
        )
        .await
        .is_err()
    );
    generation
        .publish_search_materialization("key".into(), Ok(serde_json::json!({"hits": 1})))
        .unwrap();
    // Models publication between reading Building and subscribing to its event.
    assert!(*completion.subscribe().borrow());
    assert!(matches!(
        generation
            .await_search_materialization("key")
            .await
            .unwrap(),
        RuntimeSearchMaterializationState::Ready(_)
    ));
}

#[test]
fn search_materialization_failure_is_terminal_and_generation_local() {
    let older = test_generation("blake3-256:generation-old");
    let newer = test_generation("blake3-256:generation-new");
    let key = "blake3-256:same-query".to_owned();

    assert!(older.begin_search_materialization(key.clone()).unwrap());
    older
        .publish_search_materialization(
            key.clone(),
            Err(agent_semantic_client_server::AspClientDispatchError {
                reason_kind: "search-materialization-failed".to_owned(),
                message: "fixture failure".to_owned(),
                details: None,
            }),
        )
        .unwrap();
    let Some(RuntimeSearchMaterializationState::Failed(error)) =
        older.search_materialization(&key).unwrap()
    else {
        panic!("failed materialization must remain terminal")
    };
    assert_eq!(error.reason_kind, "search-materialization-failed");

    assert!(newer.search_materialization(&key).unwrap().is_none());
    assert!(newer.begin_search_materialization(key).unwrap());
}

#[test]
fn query_materialization_claims_once_and_preserves_a_generation_local_terminal() {
    let generation = test_generation("blake3-256:query-generation");
    let other_generation = test_generation("blake3-256:other-query-generation");
    let key = "blake3-256:query-materialization".to_owned();

    assert!(generation.begin_query_materialization(key.clone()).unwrap());
    assert!(!generation.begin_query_materialization(key.clone()).unwrap());
    assert!(matches!(
        generation.query_materialization(&key).unwrap(),
        Some(RuntimeQueryMaterializationState::Building(_))
    ));
    let template = serde_json::json!({"requestId": key, "terminal": {"state": "ready"}});
    generation
        .publish_query_materialization(key.clone(), Ok(template.clone()))
        .unwrap();
    let Some(RuntimeQueryMaterializationState::Ready(published)) =
        generation.query_materialization(&key).unwrap()
    else {
        panic!("Query materialization must publish Ready")
    };
    assert_eq!(published.as_ref(), &template);
    assert!(
        other_generation
            .query_materialization(&key)
            .unwrap()
            .is_none()
    );
    assert!(
        generation
            .publish_query_materialization(key, Ok(template))
            .is_err()
    );
}

#[test]
fn search_playbook_readiness_rejects_generation_without_topology_attachment() {
    let generation = test_generation("blake3-256:missing-topology");
    let error = generation
        .require_search_playbook_topology_attachment()
        .expect_err("flat resident evidence cannot mint a Search GQL settlement");
    assert_eq!(
        error,
        "reasonKind=runtime-project-topology-attachment-missing"
    );
}

#[tokio::test]
async fn republishing_old_arc_cannot_mint_or_rollback() {
    let authority = authority();
    let workspace_key = key("project-test", "workspace-test");
    let old = test_generation("blake3-256:old");
    let newer = test_generation("blake3-256:newer");
    let old_token = authority
        .publish_ready_fixture(workspace_key.clone(), std::sync::Arc::clone(&old))
        .expect("first publication");
    let newer_token = authority
        .publish_ready_fixture(workspace_key.clone(), std::sync::Arc::clone(&newer))
        .expect("newer publication");
    assert!(newer_token > old_token);
    assert!(
        authority
            .publish_ready_fixture(workspace_key.clone(), old)
            .is_err()
    );
    let current = authority.subscribe();
    let snapshot = current.borrow().clone();
    let RuntimeQueryGenerationState::Ready(current) =
        snapshot.get(&workspace_key).expect("current")
    else {
        panic!("expected ready generation")
    };
    assert_eq!(current.generation_digest(), "blake3-256:newer");
    assert_eq!(current.generation_token(), newer_token);
}

#[tokio::test]
async fn ready_publication_rejects_a_generation_without_a_resident_base() {
    let authority = authority();
    let workspace_key = key("project-test", "workspace-test");
    let incomplete = test_generation("blake3-256:missing-resident");
    let error = authority
        .publish_ready(workspace_key.clone(), incomplete)
        .expect_err("Ready is not visible before the resident base is attached");
    assert_eq!(
        error,
        "state=query-not-ready reasonKind=resident-generation-missing"
    );
    assert!(authority.subscribe().borrow().get(&workspace_key).is_none());
}

#[tokio::test]
async fn identical_workspace_ids_in_distinct_projects_never_alias() {
    let authority = authority();
    let project_a = key("project-a", "workspace-shared");
    let project_b = key("project-b", "workspace-shared");
    authority.publish_failed(
        project_a.clone(),
        0,
        "blake3-256:expected-a",
        "fixture failure",
    );

    assert!(matches!(
        authority.subscribe().borrow().get(&project_a),
        Some(RuntimeQueryGenerationState::Failed { .. })
    ));
    assert!(authority.subscribe().borrow().get(&project_b).is_none());
    authority
        .require_workspace_absent(&project_b)
        .expect("ProjectId partitions identical WorkspaceId values");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn runtime_builder_owns_independent_non_blocking_derived_jobs() {
    use std::sync::Arc;
    use std::sync::Barrier;
    use std::sync::atomic::AtomicUsize;
    use std::sync::atomic::Ordering;
    use std::sync::mpsc;
    use std::time::Duration;

    let task_scope = agent_semantic_client_db::runtime_server_runtime::RuntimeServerTaskScope::new(
        "runtime-search-generation-builder-test",
    );
    let builder = Arc::new(
        RuntimeSearchGenerationBuilder::new(
            task_scope.clone(),
            agent_semantic_client_db::runtime_server_runtime::RuntimeServerResourceSupervisor::new(
                4,
                256 * 1024 * 1024,
            ),
        )
        .expect("test Runtime-owned search generation builder"),
    );
    let mut attachment_events = builder.subscribe_attachment_events();
    let graph_gate = Arc::new(Barrier::new(2));
    let lexical_gate = Arc::new(Barrier::new(2));
    let completed = Arc::new(AtomicUsize::new(0));
    let (started_sender, started_receiver) = mpsc::channel();

    let graph_started = started_sender.clone();
    let graph_completed = Arc::clone(&completed);
    let graph_worker_gate = Arc::clone(&graph_gate);
    let lexical_started = started_sender;
    let lexical_completed = Arc::clone(&completed);
    let lexical_worker_gate = Arc::clone(&lexical_gate);
    let joint_builder = Arc::clone(&builder);
    let joint_terminal = tokio::spawn(async move {
        joint_builder
            .build_job_and_wait(RuntimeSearchGenerationBuildJob {
                graph: RuntimeSearchGenerationBuildOperation {
                    name: "test-graph-build",
                    identity: attachment_identity(RuntimeSearchDerivedAttachmentKind::Graph),
                    resources: agent_semantic_client_db::runtime_server_runtime::RuntimeServerResourceRequest {
                        cpu: 1,
                        memory_bytes: 1024 * 1024,
                    },
                    build: Box::new(move || {
                        graph_started.send("graph").expect("graph start receipt");
                        graph_worker_gate.wait();
                        graph_completed.fetch_add(1, Ordering::AcqRel);
                        Ok(agent_semantic_client_db::runtime_server_workspace::RuntimeDerivedAttachmentBuildTiming::default())
                    }),
                    fail: Box::new(|_| {}),
                },
                lexical: RuntimeSearchGenerationBuildOperation {
                    name: "test-lexical-build",
                    identity: attachment_identity(RuntimeSearchDerivedAttachmentKind::Tantivy),
                    resources: agent_semantic_client_db::runtime_server_runtime::RuntimeServerResourceRequest {
                        cpu: 1,
                        memory_bytes: 1024 * 1024,
                    },
                    build: Box::new(move || {
                        lexical_started
                            .send("lexical")
                            .expect("lexical start receipt");
                        lexical_worker_gate.wait();
                        lexical_completed.fetch_add(1, Ordering::AcqRel);
                        Ok(agent_semantic_client_db::runtime_server_workspace::RuntimeDerivedAttachmentBuildTiming::default())
                    }),
                    fail: Box::new(|_| {}),
                },
            })
            .await
    });

    let first = started_receiver
        .recv_timeout(Duration::from_secs(2))
        .expect("first derived builder starts");
    let second = started_receiver
        .recv_timeout(Duration::from_secs(2))
        .expect("second derived builder starts independently");
    assert_ne!(first, second);
    assert_eq!(completed.load(Ordering::Acquire), 0);

    graph_gate.wait();
    tokio::time::timeout(Duration::from_secs(2), async {
        while completed.load(Ordering::Acquire) != 1 {
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("graph build completes without lexical build");

    lexical_gate.wait();
    tokio::time::timeout(Duration::from_secs(2), async {
        while completed.load(Ordering::Acquire) != 2 {
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("lexical build completes independently");
    tokio::time::timeout(Duration::from_secs(2), async {
        use tokio_stream::StreamExt;

        let mut states = std::collections::BTreeMap::<_, Vec<_>>::new();
        while states.values().map(Vec::len).sum::<usize>() < 6 {
            let event = attachment_events
                .next()
                .await
                .expect("attachment event stream remains open")
                .expect("bounded attachment event stream does not lag");
            assert_eq!(event.schema_version, "1");
            assert_eq!(
                event.content_generation_digest,
                format!("blake3-256:{}", "a".repeat(64))
            );
            assert!(event.sequence > 0);
            states
                .entry(event.attachment)
                .or_default()
                .push(event.state);
        }
        let expected = vec![
            RuntimeSearchDerivedAttachmentState::Queued,
            RuntimeSearchDerivedAttachmentState::Building,
            RuntimeSearchDerivedAttachmentState::Ready,
        ];
        assert_eq!(states[&RuntimeSearchDerivedAttachmentKind::Graph], expected);
        assert_eq!(
            states[&RuntimeSearchDerivedAttachmentKind::Tantivy],
            expected
        );
    })
    .await
    .expect("exact-generation attachment stream emits every independent transition");
    joint_terminal
        .await
        .expect("joint generation terminal task joins")
        .expect("all derived attachments terminalize together");
    let snapshot = builder.derived_attachment_snapshot();
    assert_eq!(snapshot.len(), 2);
    assert!(
        snapshot
            .values()
            .all(|event| event.state == RuntimeSearchDerivedAttachmentState::Ready)
    );
    builder.shutdown().await.expect("builder drain and join");
    let receipt = task_scope.finish(0).expect("owned task scope drained");
    assert_eq!(receipt.active, 0);
    assert_eq!(receipt.leaked, 0);
    assert_eq!(
        receipt.started, 2,
        "the caller-owned joint terminal admits only graph and Tantivy blocking tasks"
    );
    assert_eq!(receipt.started, receipt.completed);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn derived_generation_terminal_waits_for_every_attachment() {
    use std::sync::Arc;
    use std::sync::Barrier;
    use std::sync::mpsc;
    use std::time::Duration;

    let task_scope = agent_semantic_client_db::runtime_server_runtime::RuntimeServerTaskScope::new(
        "runtime-search-generation-atomic-terminal-test",
    );
    let builder = Arc::new(
        RuntimeSearchGenerationBuilder::new(
            task_scope.clone(),
            agent_semantic_client_db::runtime_server_runtime::RuntimeServerResourceSupervisor::new(
                4,
                256 * 1024 * 1024,
            ),
        )
        .expect("test Runtime-owned search generation builder"),
    );
    let graph_gate = Arc::new(Barrier::new(2));
    let lexical_gate = Arc::new(Barrier::new(2));
    let (started_sender, started_receiver) = mpsc::channel();
    let graph_started = started_sender.clone();
    let lexical_started = started_sender;
    let graph_worker_gate = Arc::clone(&graph_gate);
    let lexical_worker_gate = Arc::clone(&lexical_gate);
    let terminal_builder = Arc::clone(&builder);
    let terminal = tokio::spawn(async move {
        terminal_builder
            .build_job_and_wait(RuntimeSearchGenerationBuildJob {
                graph: RuntimeSearchGenerationBuildOperation {
                    name: "atomic-terminal-graph",
                    identity: attachment_identity(RuntimeSearchDerivedAttachmentKind::Graph),
                    resources: agent_semantic_client_db::runtime_server_runtime::RuntimeServerResourceRequest {
                        cpu: 1,
                        memory_bytes: 1024 * 1024,
                    },
                    build: Box::new(move || {
                        graph_started.send(()).expect("graph start receipt");
                        graph_worker_gate.wait();
                        Ok(agent_semantic_client_db::runtime_server_workspace::RuntimeDerivedAttachmentBuildTiming::default())
                    }),
                    fail: Box::new(|_| {}),
                },
                lexical: RuntimeSearchGenerationBuildOperation {
                    name: "atomic-terminal-lexical",
                    identity: attachment_identity(RuntimeSearchDerivedAttachmentKind::Tantivy),
                    resources: agent_semantic_client_db::runtime_server_runtime::RuntimeServerResourceRequest {
                        cpu: 1,
                        memory_bytes: 1024 * 1024,
                    },
                    build: Box::new(move || {
                        lexical_started.send(()).expect("lexical start receipt");
                        lexical_worker_gate.wait();
                        Ok(agent_semantic_client_db::runtime_server_workspace::RuntimeDerivedAttachmentBuildTiming::default())
                    }),
                    fail: Box::new(|_| {}),
                },
            })
            .await
    });

    started_receiver
        .recv_timeout(Duration::from_secs(2))
        .expect("first attachment starts");
    started_receiver
        .recv_timeout(Duration::from_secs(2))
        .expect("second attachment starts");
    assert!(!terminal.is_finished());
    graph_gate.wait();
    tokio::time::sleep(Duration::from_millis(10)).await;
    assert!(
        !terminal.is_finished(),
        "one attachment cannot terminalize the generation"
    );
    lexical_gate.wait();
    terminal
        .await
        .expect("terminal task joins")
        .expect("both attachments admit one generation terminal");

    builder.shutdown().await.expect("builder drain and join");
    let receipt = task_scope.finish(0).expect("owned task scope drained");
    assert_eq!(receipt.active, 0);
    assert_eq!(receipt.leaked, 0);
}
