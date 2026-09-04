use agent_semantic_client_db::runtime_server_admission::WorkspaceGenerationBuildFailure;
use agent_semantic_client_db::runtime_server_admission::WorkspaceGenerationFailureStage;
use std::sync::Arc;

use agent_semantic_client_db::runtime_server_admission::WORKSPACE_GENERATION_ADMISSION_RECEIPT_SCHEMA_ID;
use agent_semantic_client_db::runtime_server_admission::WORKSPACE_GENERATION_MUTATION_ADMISSION_RECEIPT_SCHEMA_ID;
use agent_semantic_client_db::runtime_server_admission::WorkspaceGenerationAdmission;
use agent_semantic_client_db::runtime_server_admission::WorkspaceGenerationAdmissionReceipt;
use agent_semantic_client_db::runtime_server_admission::WorkspaceGenerationAdmissionState;
use agent_semantic_client_db::runtime_server_admission::WorkspaceGenerationBuildMode;
use agent_semantic_client_db::runtime_server_admission::WorkspaceGenerationCandidateIdentity;
use agent_semantic_client_db::runtime_server_admission::WorkspaceGenerationMutationAdmissionReceipt;
use agent_semantic_client_db::runtime_server_admission_catalog::RuntimeWorkspaceAdmissionCatalog;
use agent_semantic_client_db::runtime_server_admission_catalog::RuntimeWorkspaceAdmissionCatalogEntry;
use agent_semantic_client_db::runtime_telemetry_bus::RuntimeTelemetryBus;
use agent_semantic_client_db::runtime_telemetry_bus::RuntimeTelemetryEvent;
use tokio::sync::Barrier;
use tokio::sync::Mutex;

pub(super) fn candidate_identity() -> WorkspaceGenerationCandidateIdentity {
    candidate_identity_for(
        "blake3:cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc",
    )
}

fn candidate_identity_for(digest: &str) -> WorkspaceGenerationCandidateIdentity {
    WorkspaceGenerationCandidateIdentity {
        candidate_generation: agent_semantic_runtime::git::RepositoryCandidateGeneration {
            algorithm: "blake3-worktree-state-v1".to_owned(),
            digest: digest.to_owned(),
            authorities: vec![agent_semantic_runtime::git::RepositoryCandidateAuthority::GitIndex],
        },
        policy_overlay_digest:
            "blake3:dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd".to_owned(),
    }
}

fn ready_receipt(workspace_identity: &str) -> WorkspaceGenerationAdmissionReceipt {
    WorkspaceGenerationAdmissionReceipt {
        trigger: agent_semantic_client_db::runtime_server_admission::WorkspaceGenerationAdmissionTrigger::QueryDemand,
        admission_mode: agent_semantic_client_db::runtime_server_admission::WorkspaceGenerationAdmissionMode::CompleteGeneration,
        build_owner: "runtime-server".to_owned(),
        cancellation_authority: "runtime-server".to_owned(),
        request_lifetime_independent: true,
        schema_id: WORKSPACE_GENERATION_ADMISSION_RECEIPT_SCHEMA_ID.to_owned(),
        schema_version: "1".to_owned(),
        workspace_identity: workspace_identity.to_owned(),
        candidate_generation: candidate_identity().candidate_generation,
        policy_overlay_digest: candidate_identity().policy_overlay_digest,
        state: WorkspaceGenerationAdmissionState::Ready,
        accepted: true,
        failure_stage: None,
        attempt: 1,
        commit: Some(
            agent_semantic_client_db::runtime_server_admission::WorkspaceGenerationCommitReceipt {
    projection_capability: agent_semantic_client_db::active_generation_projection_capability::ActiveGenerationProjectionCapabilityManifest::single_selector("blake3-256:0000000000000000000000000000000000000000000000000000000000000000".to_owned(), "rust://fixture/src/lib.rs#item/function/fixture".to_owned(), "src/lib.rs".to_owned(), std::collections::BTreeSet::from([agent_semantic_client_db::active_generation_projection_capability::ActiveGenerationProjectionMode::Source])).expect("test projection capability manifest").into_ready_receipt("workspace-test".to_owned(), "blake3-256:1111111111111111111111111111111111111111111111111111111111111111".to_owned(), "blake3-256:2222222222222222222222222222222222222222222222222222222222222222".to_owned(), 1).expect("test projection capability receipt"),
                active_epoch: 1,
                generation_digest:
                    "blake3-256:1111111111111111111111111111111111111111111111111111111111111111"
                        .to_owned(),
                source_root_digest:
                    "blake3-256:2222222222222222222222222222222222222222222222222222222222222222"
                        .to_owned(),
            },
        ),
        error: None,
    }
}

fn committed_generation()
-> agent_semantic_client_db::runtime_server_admission::WorkspaceGenerationCommitReceipt {
    agent_semantic_client_db::runtime_server_admission::WorkspaceGenerationCommitReceipt {
    projection_capability: agent_semantic_client_db::active_generation_projection_capability::ActiveGenerationProjectionCapabilityManifest::single_selector("blake3-256:0000000000000000000000000000000000000000000000000000000000000000".to_owned(), "rust://fixture/src/lib.rs#item/function/fixture".to_owned(), "src/lib.rs".to_owned(), std::collections::BTreeSet::from([agent_semantic_client_db::active_generation_projection_capability::ActiveGenerationProjectionMode::Source])).expect("test projection capability manifest").into_ready_receipt("workspace-test".to_owned(), "blake3-256:1111111111111111111111111111111111111111111111111111111111111111".to_owned(), "blake3-256:2222222222222222222222222222222222222222222222222222222222222222".to_owned(), 1).expect("test projection capability receipt"),
        active_epoch: 1,
        generation_digest:
            "blake3-256:1111111111111111111111111111111111111111111111111111111111111111".to_owned(),
        source_root_digest:
            "blake3-256:2222222222222222222222222222222222222222222222222222222222222222".to_owned(),
    }
}

fn completed_generation(
    candidate: agent_semantic_client_db::runtime_server_admission::WorkspaceGenerationCandidateIdentity,
) -> Result<
    agent_semantic_client_db::runtime_server_admission::WorkspaceGenerationBuildCompletion,
    agent_semantic_client_db::runtime_server_admission::WorkspaceGenerationBuildFailure,
> {
    agent_semantic_client_db::runtime_server_admission::WorkspaceGenerationBuildCompletion::new(
        candidate,
        committed_generation(),
    )
    .map_err(|error| agent_semantic_client_db::runtime_server_admission::WorkspaceGenerationBuildFailure::new(
        agent_semantic_client_db::runtime_server_admission::WorkspaceGenerationFailureStage::CanonicalGenerationPublication,
        error,
    ))
}

#[test]
fn ready_admission_requires_generation_commit_evidence() {
    let mut missing = ready_receipt("workspace-ready-commit");
    missing.commit = None;
    assert_eq!(
        missing.validate().unwrap_err(),
        "workspace generation admission receipt state is inconsistent"
    );

    let committed = ready_receipt("workspace-ready-commit");
    committed.validate().expect("committed Ready receipt");
}

#[tokio::test]
async fn enqueue_is_queued_until_dispatcher_publishes_one_build_started_event() {
    let build_invoked = Arc::new(tokio::sync::Notify::new());
    let release = Arc::new(tokio::sync::Notify::new());
    let bus = RuntimeTelemetryBus::new();
    let mut telemetry = bus.receiver;
    let admission = WorkspaceGenerationAdmission::new_with_telemetry_sender(
        Arc::new({
            let build_invoked = Arc::clone(&build_invoked);
            let release = Arc::clone(&release);
            move |_workspace_identity,
                  _project_root,
                  candidate,
                  _build_mode,
                  _changed_paths,
                  _provider_target,
                  _cancellation| {
                let build_invoked = Arc::clone(&build_invoked);
                let release = Arc::clone(&release);
                Box::pin(async move {
                    build_invoked.notify_one();
                    release.notified().await;
                    completed_generation(candidate)
                })
            }
        }),
        bus.sender,
    );
    let project_root = std::env::temp_dir().join("asp-generation-queued-dispatch");

    let queued = admission
        .admit(
            "workspace-queued-dispatch",
            project_root.clone(),
            candidate_identity(),
        )
        .await
        .expect("enqueue generation");
    assert_eq!(queued.state, WorkspaceGenerationAdmissionState::Queued);
    assert_eq!(queued.attempt, 1);
    assert_eq!(
        queued.candidate_generation.digest,
        candidate_identity().candidate_generation.digest
    );
    assert!(queued.accepted);

    let started = loop {
        match telemetry
            .recv()
            .await
            .expect("generation build-started event")
        {
            RuntimeTelemetryEvent::Lifecycle(event)
                if event.transition == "generation-build-started" =>
            {
                break event;
            }
            RuntimeTelemetryEvent::Lifecycle(_)
            | RuntimeTelemetryEvent::SearchIncident(_)
            | RuntimeTelemetryEvent::Performance(_) => {}
        }
    };
    assert_eq!(started.owner_epoch, 1);
    assert_eq!(started.state, "building");
    assert_eq!(started.generation_digest, None);
    assert_eq!(
        started.candidate_digest,
        Some(candidate_identity().candidate_generation.digest)
    );
    assert_eq!(started.active_task_count, 1);
    assert_eq!(
        admission
            .current("workspace-queued-dispatch", &project_root)
            .expect("resident admission")
            .state,
        WorkspaceGenerationAdmissionState::Building
    );
    build_invoked.notified().await;
    release.notify_one();

    let terminal = admission
        .wait_terminal("workspace-queued-dispatch", &project_root)
        .await
        .expect("terminal generation");
    assert_eq!(terminal.state, WorkspaceGenerationAdmissionState::Ready);
    let mut build_started_count = 1;
    while let Ok(event) = telemetry.try_recv() {
        if matches!(event, RuntimeTelemetryEvent::Lifecycle(ref event) if event.transition == "generation-build-started")
        {
            build_started_count += 1;
        }
    }
    assert_eq!(build_started_count, 1);
    admission.shutdown().await.expect("shutdown admission");
}

#[test]
fn empty_source_index_projection_cannot_publish_ready_generation() {
    let writer = include_str!("../../src/runtime_server_workspace/registry/writer_publication.rs");
    let canonical =
        include_str!("../../src/runtime_server_workspace/registry/canonical_publication.rs");
    for source in [writer, canonical] {
        assert!(source.contains("source-index completeness gate rejected publication"));
        assert!(source.contains("generation.owners.is_empty()"));
        assert!(source.contains("reasonKind=source-index-resident-index-missing"));
    }
}

#[test]
fn cold_restore_publishes_committed_generation_without_live_checkout_probe() {
    let source = include_str!("../../src/runtime_server/core.rs");
    assert!(source.contains("let captured_candidate = build.candidate.clone();"));
    assert!(source.contains(
        "WorkspaceGenerationBuildCompletion::new(\n                        captured_candidate,"
    ));
    assert!(!source.contains("canonical_materialization_matches_candidate_generation"));
    assert!(!source.contains("discover_repository_candidate_snapshot"));
    assert!(!source.contains("if materialization.project_resolutions.is_empty()"));
}

#[test]
fn cold_byte_generation_is_published_before_source_index_durability_attachment() {
    let source = include_str!("../../src/runtime_server/core.rs");
    assert!(source.contains("let durable_restore_admitted ="));
    assert!(source.contains("durable_provider_binding_matches_current("));
    let publication = source
        .find("generation_publication.publish(")
        .expect("canonical resident generation publication");
    let completion = source
        .find("let completion = crate::runtime_server_admission::WorkspaceGenerationBuildCompletion::new(")
        .expect("canonical admission completion");
    let durability_task = source
        .find("spawn_runtime_owned_durability_task(&durability_tasks, async move")
        .expect("Runtime-owned durability attachment task");
    let durability = source
        .find(".commit_source_index_generation(")
        .expect("Source Index durability attachment");
    let workspace_bootstrap = source
        .find(".bootstrap_workspace(&project_root)")
        .expect("workspace durability bootstrap");
    assert!(
        publication < completion
            && completion < durability_task
            && durability_task < workspace_bootstrap
            && workspace_bootstrap < durability,
        "Turso bootstrap and commit must both run only inside the post-publication durability task"
    );
    assert!(!source.contains("tokio::spawn(async move"));
    assert!(source.contains("agent.semantic-protocols.source-index-durability-attachment-receipt"));
    assert!(source.contains("source-index-durability-attachment-failed"));
}

#[test]
fn concurrent_256_requests_share_one_server_workspace_writer_lease() {
    let runtime =
        agent_semantic_client_db::runtime_server_runtime::RuntimeServerRuntimeBuilder::new_daemon()
            .enable_all()
            .build()
            .expect("adaptive Runtime Server daemon runtime");
    runtime.block_on(async {
        const REQUEST_COUNT: usize = 256;
        let build_count = Arc::new(Mutex::new(0_u32));
        let release = Arc::new(Barrier::new(2));
        let admission = WorkspaceGenerationAdmission::new(Arc::new({
            let build_count = Arc::clone(&build_count);
            let release = Arc::clone(&release);
            move |_workspace_identity,
                  _project_root,
                  candidate,
                  _build_mode,
                  _changed_paths,
                  _provider_target,
                  _cancellation| {
                let build_count = Arc::clone(&build_count);
                let release = Arc::clone(&release);
                Box::pin(async move {
                    *build_count.lock().await += 1;
                    release.wait().await;
                    completed_generation(candidate)
                })
            }
        }));

        let project_root = std::env::temp_dir().join("asp-generation-admission-project");
        let mut requests = tokio::task::JoinSet::new();
        for _ in 0..REQUEST_COUNT {
            let admission = admission.clone();
            let project_root = project_root.clone();
            requests.spawn(async move {
                let started = tokio::time::Instant::now();
                let receipt = admission
                    .admit(
                        "workspace-server-writer-lease",
                        project_root,
                        candidate_identity(),
                    )
                    .await;
                (receipt, started.elapsed())
            });
        }
        let mut accepted_count = 0_u32;
        let mut admission_latencies = Vec::with_capacity(REQUEST_COUNT);
        while let Some(result) = requests.join_next().await {
            let (receipt, elapsed) = result.expect("admission request task");
            let receipt = receipt.expect("admission receipt");
            admission_latencies.push(elapsed);
            assert!(matches!(
                receipt.state,
                WorkspaceGenerationAdmissionState::Queued
                    | WorkspaceGenerationAdmissionState::Building
            ));
            accepted_count += u32::from(receipt.accepted);
            receipt.validate().expect("valid admission receipt");
        }
        admission_latencies.sort_unstable();
        let p99_index = admission_latencies.len().saturating_mul(99).div_ceil(100) - 1;
        let p99 = admission_latencies[p99_index];
        assert!(
            p99 < std::time::Duration::from_millis(1),
            "generation admission submit p99 must remain sub-millisecond: {p99:?}"
        );
        release.wait().await;
        assert_eq!(accepted_count, 1);
        assert_eq!(*build_count.lock().await, 1);
        let receipt = admission
            .wait_terminal("workspace-server-writer-lease", &project_root)
            .await
            .expect("wait for completed admission");
        assert_eq!(receipt.state, WorkspaceGenerationAdmissionState::Ready);
        admission
            .shutdown()
            .await
            .expect("drain generation admission lane");
    });
}

#[path = "runtime_server_generation_admission_performance.rs"]
mod runtime_server_generation_admission_performance;

#[tokio::test]
async fn project_roots_have_independent_admission_flights() {
    let roots = Arc::new(Mutex::new(Vec::new()));
    let admission = WorkspaceGenerationAdmission::new(Arc::new({
        let roots = Arc::clone(&roots);
        move |_workspace_identity,
              project_root,
              candidate,
              _build_mode,
              _changed_paths,
              _provider_target,
              _cancellation| {
            let roots = Arc::clone(&roots);
            Box::pin(async move {
                roots.lock().await.push(project_root);
                completed_generation(candidate)
            })
        }
    }));
    let first_root = std::env::temp_dir().join("asp-generation-admission-first-project");
    let second_root = std::env::temp_dir().join("asp-generation-admission-second-project");

    let first = admission
        .admit(
            "shared-repository-identity",
            first_root.clone(),
            candidate_identity(),
        )
        .await
        .expect("admit first project root");
    let second = admission
        .admit(
            "shared-repository-identity",
            second_root.clone(),
            candidate_identity(),
        )
        .await
        .expect("admit second project root");
    assert!(first.accepted);
    assert!(second.accepted);
    assert_eq!(
        admission
            .wait_terminal("shared-repository-identity", &first_root)
            .await
            .expect("first project terminal state")
            .state,
        WorkspaceGenerationAdmissionState::Ready
    );
    assert_eq!(
        admission
            .wait_terminal("shared-repository-identity", &second_root)
            .await
            .expect("second project terminal state")
            .state,
        WorkspaceGenerationAdmissionState::Ready
    );
    let mut observed_roots = roots.lock().await.clone();
    observed_roots.sort();
    assert_eq!(observed_roots, vec![first_root, second_root]);

    admission
        .shutdown()
        .await
        .expect("drain independent admission lanes");
}

#[tokio::test(flavor = "multi_thread")]
async fn multi_workspace_admission_uses_independent_server_writer_leases_and_is_sub_millisecond() {
    let _performance = crate::test_support::performance_lock();
    let machine_parallelism = std::thread::available_parallelism()
        .map(std::num::NonZeroUsize::get)
        .unwrap_or(2);
    let workspace_count = machine_parallelism.max(2);
    let calls_per_workspace = machine_parallelism.saturating_mul(16).max(32);
    let active_builds = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let peak_builds = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let build_count = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let release = Arc::new(tokio::sync::Semaphore::new(0));
    let admission = Arc::new(WorkspaceGenerationAdmission::new(Arc::new({
        let active_builds = Arc::clone(&active_builds);
        let peak_builds = Arc::clone(&peak_builds);
        let build_count = Arc::clone(&build_count);
        let release = Arc::clone(&release);
        move |_, _, candidate, _, _changed_paths, _provider_target, _cancellation| {
            let active_builds = Arc::clone(&active_builds);
            let peak_builds = Arc::clone(&peak_builds);
            let build_count = Arc::clone(&build_count);
            let release = Arc::clone(&release);
            Box::pin(async move {
                build_count.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                let active = active_builds.fetch_add(1, std::sync::atomic::Ordering::AcqRel) + 1;
                peak_builds.fetch_max(active, std::sync::atomic::Ordering::AcqRel);
                release
                    .acquire_owned()
                    .await
                    .map_err(|_| {
                        WorkspaceGenerationBuildFailure::new(
                            WorkspaceGenerationFailureStage::GenerationBuilder,
                            "in-process admission release closed",
                        )
                    })?
                    .forget();
                active_builds.fetch_sub(1, std::sync::atomic::Ordering::AcqRel);
                completed_generation(candidate)
            })
        }
    })));

    let mut requests = tokio::task::JoinSet::new();
    for workspace_index in 0..workspace_count {
        for _ in 0..calls_per_workspace {
            let admission = Arc::clone(&admission);
            requests.spawn(async move {
                let workspace_identity = format!("workspace-{workspace_index}");
                let root = std::env::temp_dir().join(&workspace_identity);
                let started = tokio::time::Instant::now();
                let receipt = admission
                    .admit_observed_mutation(
                        format!("mutation-{workspace_index}"),
                        workspace_identity,
                        root.clone(),
                        vec![root.join("src/lib.rs")],
                        candidate_identity(),
                    )
                    .await;
                (workspace_index, receipt, started.elapsed())
            });
        }
    }

    let mut accepted_per_workspace = vec![0_usize; workspace_count];
    let mut latencies = Vec::with_capacity(workspace_count * calls_per_workspace);
    while let Some(joined) = requests.join_next().await {
        let (workspace_index, receipt, latency) = joined.expect("join in-process admission");
        let receipt = receipt.expect("admit in-process workspace mutation");
        accepted_per_workspace[workspace_index] += usize::from(
            receipt
                .receipts
                .first()
                .is_some_and(|receipt| receipt.accepted),
        );
        latencies.push(latency);
    }
    assert!(accepted_per_workspace.iter().all(|accepted| *accepted == 1));
    latencies.sort_unstable();
    let p50 = latencies[(latencies.len() * 50 / 100).min(latencies.len() - 1)];
    let p95 = latencies[(latencies.len() * 95 / 100).min(latencies.len() - 1)];
    let p99 = latencies[(latencies.len() * 99 / 100).min(latencies.len() - 1)];
    let max = *latencies.last().expect("admission latency sample");
    eprintln!(
        "in-process-generation-admission measurementPath=admit_observed_mutation-to-dispatcher-enqueue sampleCount={} workspaceCount={workspace_count} callsPerWorkspace={calls_per_workspace} p50Nanos={} p95Nanos={} p99Nanos={} maxNanos={}",
        latencies.len(),
        p50.as_nanos(),
        p95.as_nanos(),
        p99.as_nanos(),
        max.as_nanos(),
    );
    assert!(
        p99 < std::time::Duration::from_millis(1),
        "resident in-process admission p99 must remain sub-millisecond: {p99:?}"
    );

    tokio::time::timeout(std::time::Duration::from_millis(100), async {
        while active_builds.load(std::sync::atomic::Ordering::Acquire) < workspace_count {
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("different workspaces must enter independent writer lanes within 100ms");
    assert_eq!(
        peak_builds.load(std::sync::atomic::Ordering::Acquire),
        workspace_count
    );
    assert_eq!(
        build_count.load(std::sync::atomic::Ordering::Acquire),
        workspace_count
    );

    release.add_permits(workspace_count);
    for workspace_index in 0..workspace_count {
        let workspace_identity = format!("workspace-{workspace_index}");
        let root = std::env::temp_dir().join(&workspace_identity);
        let terminal = tokio::time::timeout(
            std::time::Duration::from_millis(100),
            admission.wait_terminal(&workspace_identity, &root),
        )
        .await
        .expect("workspace admission must reach terminal state within 100ms")
        .expect("workspace admission reaches terminal state");
        assert_eq!(terminal.state, WorkspaceGenerationAdmissionState::Ready);
    }
    tokio::time::timeout(std::time::Duration::from_millis(100), admission.shutdown())
        .await
        .expect("in-process workspace generation lanes must drain within 100ms")
        .expect("drain in-process workspace generation lanes");
}

#[path = "runtime_server_generation_reconciliation.rs"]
mod runtime_server_generation_reconciliation;

#[tokio::test]
async fn ensure_observes_ready_attempt_without_starting_another_build() {
    let build_count = Arc::new(Mutex::new(0_u32));
    let admission = WorkspaceGenerationAdmission::new(Arc::new({
        let build_count = Arc::clone(&build_count);
        move |_workspace_identity,
              _project_root,
              candidate,
              _build_mode,
              _changed_paths,
              _provider_target,
              _cancellation| {
            let build_count = Arc::clone(&build_count);
            Box::pin(async move {
                *build_count.lock().await += 1;
                completed_generation(candidate)
            })
        }
    }));
    let project_root = std::env::temp_dir().join("asp-generation-ensure-project");

    admission
        .admit(
            "workspace-ensure",
            project_root.clone(),
            candidate_identity(),
        )
        .await
        .expect("admit generation");
    admission
        .wait_terminal("workspace-ensure", &project_root)
        .await
        .expect("wait for admitted generation");
    let ready = admission
        .ensure("workspace-ensure", &project_root, candidate_identity())
        .await
        .expect("ensure admitted generation");

    assert_eq!(ready.state, WorkspaceGenerationAdmissionState::Ready);
    assert_eq!(ready.attempt, 1);
    assert_eq!(*build_count.lock().await, 1);
    admission.shutdown().await.expect("drain admission lane");
}

#[path = "runtime_server_generation_candidate_admission.rs"]
mod runtime_server_generation_candidate_admission;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn ensure_schedules_once_without_waiting_for_generation_build() {
    let build_count = Arc::new(Mutex::new(0_u32));
    let release = Arc::new(Barrier::new(2));
    let admission = WorkspaceGenerationAdmission::new(Arc::new({
        let build_count = Arc::clone(&build_count);
        let release = Arc::clone(&release);
        move |_workspace_identity,
              _project_root,
              candidate,
              _build_mode,
              _changed_paths,
              _provider_target,
              _cancellation| {
            let build_count = Arc::clone(&build_count);
            let release = Arc::clone(&release);
            Box::pin(async move {
                *build_count.lock().await += 1;
                release.wait().await;
                completed_generation(candidate)
            })
        }
    }));
    let project_root = std::env::temp_dir().join("asp-generation-ensure-submit-project");

    let first = admission
        .ensure(
            "workspace-ensure-submit",
            &project_root,
            candidate_identity(),
        )
        .await
        .expect("schedule generation");
    let second = admission
        .ensure(
            "workspace-ensure-submit",
            &project_root,
            candidate_identity(),
        )
        .await
        .expect("observe scheduled generation");

    assert_eq!(first.state, WorkspaceGenerationAdmissionState::Queued);
    assert!(matches!(
        second.state,
        WorkspaceGenerationAdmissionState::Queued | WorkspaceGenerationAdmissionState::Building
    ));
    assert!(first.accepted);
    assert!(second.accepted);
    assert_eq!(first.attempt, 1);
    assert_eq!(second.attempt, 1);
    const OBSERVATION_COUNT: usize = 4_096;
    let mut latencies = Vec::with_capacity(OBSERVATION_COUNT);
    for _ in 0..OBSERVATION_COUNT {
        let started = tokio::time::Instant::now();
        let observed = admission
            .ensure(
                "workspace-ensure-submit",
                &project_root,
                candidate_identity(),
            )
            .await
            .expect("observe resident scheduled generation");
        latencies.push(started.elapsed());
        assert!(matches!(
            observed.state,
            WorkspaceGenerationAdmissionState::Queued | WorkspaceGenerationAdmissionState::Building
        ));
        assert_eq!(observed.attempt, 1);
    }
    latencies.sort_unstable();
    let p99 = latencies[(OBSERVATION_COUNT * 99 / 100).min(OBSERVATION_COUNT - 1)];
    eprintln!(
        "resident-generation-ensure observations={OBSERVATION_COUNT} p99Nanos={} budgetNanos=1000000",
        p99.as_nanos()
    );
    assert!(
        p99 < std::time::Duration::from_millis(1),
        "resident generation ensure p99 must remain sub-millisecond: {p99:?}"
    );
    release.wait().await;
    let ready = admission
        .wait_terminal("workspace-ensure-submit", &project_root)
        .await
        .expect("wait for scheduled generation");
    assert_eq!(ready.state, WorkspaceGenerationAdmissionState::Ready);
    assert_eq!(*build_count.lock().await, 1);
    admission.shutdown().await.expect("drain admission lane");
}

#[tokio::test]
async fn failed_generation_build_retries_only_on_explicit_admission() {
    let build_count = Arc::new(Mutex::new(0_u32));
    let failed = Arc::new(tokio::sync::Notify::new());
    let admission = WorkspaceGenerationAdmission::new(Arc::new({
        let build_count = Arc::clone(&build_count);
        let failed = Arc::clone(&failed);
        move |_workspace_identity,
              _project_root,
              _candidate,
              _build_mode,
              _changed_paths,
              _provider_target,
              _cancellation| {
            let build_count = Arc::clone(&build_count);
            let failed = Arc::clone(&failed);
            Box::pin(async move {
                *build_count.lock().await += 1;
                failed.notify_one();
                Err(WorkspaceGenerationBuildFailure::new(
                    WorkspaceGenerationFailureStage::SourceBuilder,
                    "provider generation unavailable",
                ))
            })
        }
    }));
    let project_root = std::env::temp_dir().join("asp-generation-admission-failure");

    admission
        .admit(
            "workspace-sticky-failure",
            project_root.clone(),
            candidate_identity(),
        )
        .await
        .expect("schedule failed builder");
    failed.notified().await;
    let receipt = admission
        .wait_terminal("workspace-sticky-failure", &project_root)
        .await
        .expect("failure state must become visible");

    assert_eq!(receipt.state, WorkspaceGenerationAdmissionState::Failed);
    assert!(!receipt.accepted);
    assert_eq!(
        receipt.error.as_deref(),
        Some("provider generation unavailable")
    );
    assert_eq!(*build_count.lock().await, 1);
    receipt.validate().expect("valid failure receipt");

    let retry = admission
        .admit(
            "workspace-sticky-failure",
            project_root,
            candidate_identity(),
        )
        .await
        .expect("explicitly retry failed admission");
    assert_eq!(retry.state, WorkspaceGenerationAdmissionState::Queued);
    assert!(retry.accepted);
    assert_eq!(retry.attempt, 2);
    failed.notified().await;
    assert_eq!(*build_count.lock().await, 2);

    admission
        .shutdown()
        .await
        .expect("drain failed generation admission lane");
}

#[path = "runtime_server_generation_admission_failure.rs"]
mod runtime_server_generation_admission_failure;
#[path = "runtime_server_generation_admission_mutation.rs"]
mod runtime_server_generation_admission_mutation;
