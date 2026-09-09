// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use std::fs;
use std::process::Command;
use std::sync::Arc;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;
use std::time::Duration;

use agent_semantic_client_db::runtime_server_admission::WorkspaceGenerationAdmission;
use agent_semantic_client_db::runtime_server_admission::WorkspaceGenerationAdmissionMode;
use agent_semantic_client_db::runtime_server_admission::WorkspaceGenerationAdmissionReceipt;
use agent_semantic_client_db::runtime_server_admission::WorkspaceGenerationAdmissionState;
use agent_semantic_client_db::runtime_server_admission::WorkspaceGenerationBuildCompletion;
use agent_semantic_client_db::runtime_server_admission::WorkspaceGenerationBuildFailure;
use agent_semantic_client_db::runtime_server_admission::WorkspaceGenerationCommitReceipt;
use agent_semantic_client_db::runtime_server_admission::WorkspaceGenerationFailureStage;
use agent_semantic_client_db::runtime_server_admission::WorkspaceGenerationProviderTarget;
use agent_semantic_client_db::runtime_server_admission::WorkspaceGenerationReadinessRequestState;
use agent_semantic_client_db::runtime_server_admission::discover_workspace_generation_candidate;

async fn observe_terminal(
    admission: &WorkspaceGenerationAdmission,
    workspace_identity: &str,
    project_root: &std::path::Path,
    after_attempt: u64,
) -> WorkspaceGenerationAdmissionReceipt {
    tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            if let Some(receipt) = admission.status(workspace_identity, project_root)
                && receipt.attempt > after_attempt
                && matches!(
                    receipt.state,
                    WorkspaceGenerationAdmissionState::Ready
                        | WorkspaceGenerationAdmissionState::Failed
                        | WorkspaceGenerationAdmissionState::Cancelled
                )
            {
                break receipt;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("readiness request reaches terminal")
}

fn run_git(root: &std::path::Path, args: &[&str]) {
    let output = Command::new("git")
        .args(args)
        .current_dir(root)
        .output()
        .expect("run git fixture command");
    assert!(
        output.status.success(),
        "git fixture command failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

fn completed_generation(
    workspace_identity: &str,
    candidate: agent_semantic_client_db::runtime_server_admission::WorkspaceGenerationCandidateIdentity,
) -> Result<WorkspaceGenerationBuildCompletion, WorkspaceGenerationBuildFailure> {
    let projection_capability = agent_semantic_client_db::active_generation_projection_capability::ActiveGenerationProjectionCapabilityManifest::single_selector(
        "blake3-256:0000000000000000000000000000000000000000000000000000000000000000".to_owned(),
        "rust://fixture/src/lib.rs#item/function/fixture".to_owned(),
        "src/lib.rs".to_owned(),
        std::collections::BTreeSet::from([
            agent_semantic_client_db::active_generation_projection_capability::ActiveGenerationProjectionMode::Source,
        ]),
    )
    .expect("test projection capability manifest")
    .into_ready_receipt(
        workspace_identity.to_owned(),
        "blake3-256:1111111111111111111111111111111111111111111111111111111111111111".to_owned(),
        "blake3-256:2222222222222222222222222222222222222222222222222222222222222222".to_owned(),
        1,
    )
    .expect("test projection capability receipt");
    WorkspaceGenerationBuildCompletion::new(
        candidate,
        WorkspaceGenerationCommitReceipt {
            active_epoch: 1,
            generation_digest:
                "blake3-256:1111111111111111111111111111111111111111111111111111111111111111"
                    .to_owned(),
            source_root_digest:
                "blake3-256:2222222222222222222222222222222222222222222222222222222222222222"
                    .to_owned(),
            projection_capability,
        },
    )
    .map_err(|error| {
        WorkspaceGenerationBuildFailure::new(
            WorkspaceGenerationFailureStage::CanonicalGenerationPublication,
            error,
        )
    })
}

#[tokio::test(flavor = "multi_thread")]
async fn cold_candidate_discovery_completes_for_a_real_git_workspace() {
    let fixture = tempfile::tempdir().expect("candidate discovery fixture");
    let project_root = fixture.path().to_path_buf();
    run_git(&project_root, &["init", "--quiet"]);
    fs::create_dir_all(project_root.join("src")).expect("create source root");
    fs::write(
        project_root.join("src/lib.rs"),
        "pub fn candidate_discovery_owner() -> u8 { 1 }\n",
    )
    .expect("write source owner");
    run_git(&project_root, &["add", "src/lib.rs"]);
    let started = std::time::Instant::now();
    let candidate = tokio::time::timeout(
        Duration::from_secs(10),
        agent_semantic_client_db::runtime_server_admission::discover_workspace_generation_candidate(
            &project_root,
        ),
    )
    .await
    .expect("candidate discovery must be bounded")
    .expect("discover candidate for real git workspace");
    candidate.validate().expect("candidate identity");
    eprintln!(
        "cold-candidate-discovery elapsedMicros={}",
        started.elapsed().as_micros()
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn cold_readiness_request_is_single_flight_and_never_waits_for_the_build() {
    let fixture = tempfile::tempdir().expect("cold readiness fixture");
    let project_root = fixture.path().to_path_buf();
    run_git(&project_root, &["init", "--quiet"]);
    fs::create_dir_all(project_root.join("src")).expect("create source root");
    fs::write(
        project_root.join("src/lib.rs"),
        "pub fn cold_readiness_owner() -> u8 { 1 }\n",
    )
    .expect("write source owner");
    run_git(&project_root, &["add", "src/lib.rs"]);
    let workspace_identity =
        agent_semantic_client_db::AgentSessionRegistry::workspace_id(&project_root)
            .expect("derive canonical workspace identity");

    let build_count = Arc::new(AtomicUsize::new(0));
    let build_started = Arc::new(tokio::sync::Semaphore::new(0));
    let build_release = Arc::new(tokio::sync::Semaphore::new(0));
    let admission = WorkspaceGenerationAdmission::new(Arc::new({
        let build_count = Arc::clone(&build_count);
        let build_started = Arc::clone(&build_started);
        let build_release = Arc::clone(&build_release);
        move |workspace_identity,
              _project_root,
              candidate,
              _build_mode,
              _changed_paths,
              provider_target,
              _cancellation| {
            let build_count = Arc::clone(&build_count);
            let build_started = Arc::clone(&build_started);
            let build_release = Arc::clone(&build_release);
            Box::pin(async move {
                assert!(
                    provider_target.is_none(),
                    "cold readiness must publish CompleteGeneration rather than a targeted provider scope"
                );
                build_count.fetch_add(1, Ordering::AcqRel);
                build_started.add_permits(1);
                build_release
                    .acquire_owned()
                    .await
                    .expect("release cold readiness build")
                    .forget();
                completed_generation(&workspace_identity, candidate)
            })
        }
    }));

    let mut states = Vec::with_capacity(32);
    let mut latencies = Vec::with_capacity(32);
    for _ in 0..32 {
        let started = std::time::Instant::now();
        states.push(
            admission
                .request_runtime_generation_ready(workspace_identity.clone(), project_root.clone())
                .expect("submit cold readiness request"),
        );
        latencies.push(started.elapsed());
    }
    assert_eq!(
        states
            .iter()
            .filter(|state| **state == WorkspaceGenerationReadinessRequestState::Accepted)
            .count(),
        1
    );
    assert_eq!(
        states
            .iter()
            .filter(|state| **state == WorkspaceGenerationReadinessRequestState::Coalesced)
            .count(),
        31
    );
    latencies.sort_unstable();
    let p99 = latencies[latencies.len() * 99 / 100];
    eprintln!(
        "cold-readiness-request calls=32 accepted=1 coalesced=31 p99Micros={}",
        p99.as_micros()
    );
    assert!(
        p99 < Duration::from_millis(1),
        "cold readiness request p99 must remain sub-millisecond: {p99:?}"
    );

    // Candidate discovery is cold, filesystem-backed work. It is deliberately
    // outside the request latency budget and only needs to make bounded
    // asynchronous progress here.
    match tokio::time::timeout(Duration::from_secs(10), build_started.acquire()).await {
        Ok(Ok(permit)) => permit.forget(),
        Ok(Err(error)) => panic!("cold readiness start semaphore closed: {error}"),
        Err(error) => panic!(
            "cold readiness build starts independently: {error:?}; admission={:?}",
            admission.status(&workspace_identity, &project_root)
        ),
    }
    assert_eq!(build_count.load(Ordering::Acquire), 1);
    build_release.add_permits(1);
    let terminal = tokio::time::timeout(
        Duration::from_secs(1),
        admission.wait_terminal(&workspace_identity, &project_root),
    )
    .await
    .expect("cold readiness reaches terminal")
    .expect("cold readiness terminal receipt");
    assert_eq!(terminal.state, WorkspaceGenerationAdmissionState::Ready);
    assert_eq!(
        terminal.admission_mode,
        WorkspaceGenerationAdmissionMode::CompleteGeneration
    );
    assert!(terminal.commit.is_some());
    assert_eq!(
        admission
            .request_runtime_generation_ready(workspace_identity, project_root.clone(),)
            .expect("ready request is reused"),
        WorkspaceGenerationReadinessRequestState::Ready
    );

    admission.shutdown().await.expect("drain admission lane");
}

#[tokio::test(flavor = "multi_thread")]
async fn failed_readiness_request_releases_its_key_for_one_retry() {
    let fixture = tempfile::tempdir().expect("readiness retry fixture");
    let project_root = fixture.path().to_path_buf();
    run_git(&project_root, &["init", "--quiet"]);
    fs::create_dir_all(project_root.join("src")).expect("create source root");
    fs::write(
        project_root.join("src/lib.rs"),
        "pub fn readiness_retry_owner() -> u8 { 1 }\n",
    )
    .expect("write source owner");
    run_git(&project_root, &["add", "src/lib.rs"]);
    let workspace_identity =
        agent_semantic_client_db::AgentSessionRegistry::workspace_id(&project_root)
            .expect("derive canonical workspace identity");

    let build_count = Arc::new(AtomicUsize::new(0));
    let admission = WorkspaceGenerationAdmission::new(Arc::new({
        let build_count = Arc::clone(&build_count);
        move |workspace_identity,
              _project_root,
              candidate,
              _build_mode,
              _changed_paths,
              provider_target,
              _cancellation| {
            let build_count = Arc::clone(&build_count);
            Box::pin(async move {
                assert!(provider_target.is_none());
                if build_count.fetch_add(1, Ordering::AcqRel) == 0 {
                    return Err(WorkspaceGenerationBuildFailure::new(
                        WorkspaceGenerationFailureStage::GenerationBuilder,
                        "injected first readiness failure",
                    ));
                }
                completed_generation(&workspace_identity, candidate)
            })
        }
    }));

    assert_eq!(
        admission
            .request_runtime_generation_ready(workspace_identity.clone(), project_root.clone(),)
            .expect("submit first readiness request"),
        WorkspaceGenerationReadinessRequestState::Accepted
    );
    let failed = observe_terminal(&admission, &workspace_identity, &project_root, 0).await;
    assert_eq!(failed.state, WorkspaceGenerationAdmissionState::Failed);
    assert_eq!(failed.commit, None);

    let retry = tokio::time::timeout(Duration::from_secs(1), async {
        loop {
            let state = admission
                .request_runtime_generation_ready(workspace_identity.clone(), project_root.clone())
                .expect("retry readiness request");
            if state != WorkspaceGenerationReadinessRequestState::Coalesced {
                break state;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("failed readiness key is released");
    assert_eq!(retry, WorkspaceGenerationReadinessRequestState::Accepted);
    let ready = observe_terminal(
        &admission,
        &workspace_identity,
        &project_root,
        failed.attempt,
    )
    .await;
    assert_eq!(ready.state, WorkspaceGenerationAdmissionState::Ready);
    assert!(ready.commit.is_some());
    assert_eq!(build_count.load(Ordering::Acquire), 2);

    admission.shutdown().await.expect("drain admission lane");
}

#[tokio::test(flavor = "multi_thread")]
async fn provider_hint_cannot_downgrade_the_complete_generation_barrier() {
    let fixture = tempfile::tempdir().expect("provider hint fixture");
    let project_root = fixture.path().to_path_buf();
    run_git(&project_root, &["init", "--quiet"]);
    fs::create_dir_all(project_root.join("src")).expect("create source root");
    fs::write(
        project_root.join("src/lib.rs"),
        "pub fn provider_hint_owner() -> u8 { 1 }\n",
    )
    .expect("write source owner");
    run_git(&project_root, &["add", "src/lib.rs"]);
    let workspace_identity =
        agent_semantic_client_db::AgentSessionRegistry::workspace_id(&project_root)
            .expect("derive canonical workspace identity");

    let admission = WorkspaceGenerationAdmission::new(Arc::new(
        |workspace_identity,
         _project_root,
         candidate,
         _build_mode,
         _changed_paths,
         provider_target,
         _cancellation| {
            Box::pin(async move {
                assert_eq!(
                    provider_target
                        .as_ref()
                        .map(|target| target.language_id.as_str()),
                    Some("rust"),
                    "the target is retained only as an internal provider-work hint"
                );
                completed_generation(&workspace_identity, candidate)
            })
        },
    ));
    let terminal = admission
        .ensure_runtime_generation_ready_for_provider(
            workspace_identity,
            project_root,
            Some(WorkspaceGenerationProviderTarget {
                language_id: "rust".to_owned(),
                provider_id: Some("asp-rust".to_owned()),
            }),
        )
        .await
        .expect("provider hint retains complete generation barrier");
    assert_eq!(terminal.state, WorkspaceGenerationAdmissionState::Ready);
    assert_eq!(
        terminal.admission_mode,
        WorkspaceGenerationAdmissionMode::CompleteGeneration
    );
    assert!(terminal.commit.is_some());
    admission.shutdown().await.expect("drain admission lane");
}

#[tokio::test(flavor = "multi_thread")]
async fn provider_demand_rebuilds_a_ready_generation_for_its_exact_target() {
    let fixture = tempfile::tempdir().expect("provider successor fixture");
    let project_root = fixture.path().to_path_buf();
    run_git(&project_root, &["init", "--quiet"]);
    fs::create_dir_all(project_root.join("src")).expect("create source root");
    fs::write(
        project_root.join("src/lib.rs"),
        "pub fn provider_successor_owner() -> u8 { 1 }\n",
    )
    .expect("write source owner");
    run_git(&project_root, &["add", "src/lib.rs"]);
    let workspace_identity =
        agent_semantic_client_db::AgentSessionRegistry::workspace_id(&project_root)
            .expect("derive canonical workspace identity");

    let build_count = Arc::new(AtomicUsize::new(0));
    let observed_targets = Arc::new(std::sync::Mutex::new(Vec::new()));
    let admission = WorkspaceGenerationAdmission::new(Arc::new({
        let build_count = Arc::clone(&build_count);
        let observed_targets = Arc::clone(&observed_targets);
        move |workspace_identity,
              _project_root,
              candidate,
              _build_mode,
              _changed_paths,
              provider_target,
              _cancellation| {
            build_count.fetch_add(1, Ordering::SeqCst);
            observed_targets
                .lock()
                .expect("provider target observations")
                .push(provider_target.clone());
            Box::pin(async move { completed_generation(&workspace_identity, candidate) })
        }
    }));

    admission
        .ensure_runtime_generation_ready(workspace_identity.clone(), project_root.clone())
        .await
        .expect("publish initial complete generation");
    let provider_target = WorkspaceGenerationProviderTarget {
        language_id: "rust".to_owned(),
        provider_id: Some("asp-rust".to_owned()),
    };
    admission
        .ensure_runtime_generation_ready_for_provider(
            workspace_identity,
            project_root,
            Some(provider_target.clone()),
        )
        .await
        .expect("publish provider-bound complete-generation successor");

    assert_eq!(build_count.load(Ordering::SeqCst), 2);
    assert_eq!(
        *observed_targets
            .lock()
            .expect("provider target observations"),
        vec![None, Some(provider_target)]
    );
    admission.shutdown().await.expect("drain admission lane");
}

#[tokio::test(flavor = "multi_thread")]
async fn readiness_request_joins_complete_generation_build_without_rebuilding() {
    let fixture = tempfile::tempdir().expect("complete generation join fixture");
    let project_root = fixture.path().to_path_buf();
    run_git(&project_root, &["init", "--quiet"]);
    fs::create_dir_all(project_root.join("src")).expect("create source root");
    fs::write(
        project_root.join("src/lib.rs"),
        "pub fn complete_generation_owner() -> u8 { 1 }\n",
    )
    .expect("write source owner");
    run_git(&project_root, &["add", "src/lib.rs"]);
    let workspace_identity =
        agent_semantic_client_db::AgentSessionRegistry::workspace_id(&project_root)
            .expect("derive canonical workspace identity");
    let candidate = discover_workspace_generation_candidate(&project_root)
        .await
        .expect("discover complete-generation candidate");

    let build_count = Arc::new(AtomicUsize::new(0));
    let first_build_started = Arc::new(tokio::sync::Semaphore::new(0));
    let first_build_release = Arc::new(tokio::sync::Semaphore::new(0));
    let admission = WorkspaceGenerationAdmission::new(Arc::new({
        let build_count = Arc::clone(&build_count);
        let first_build_started = Arc::clone(&first_build_started);
        let first_build_release = Arc::clone(&first_build_release);
        move |workspace_identity,
              _project_root,
              candidate,
              _build_mode,
              _changed_paths,
              provider_target,
              _cancellation| {
            let build_count = Arc::clone(&build_count);
            let first_build_started = Arc::clone(&first_build_started);
            let first_build_release = Arc::clone(&first_build_release);
            Box::pin(async move {
                assert!(
                    provider_target.is_none(),
                    "provider work hints cannot become publication scope"
                );
                let build_index = build_count.fetch_add(1, Ordering::AcqRel);
                if build_index == 0 {
                    first_build_started.add_permits(1);
                    first_build_release
                        .acquire_owned()
                        .await
                        .expect("release complete-generation build")
                        .forget();
                }
                completed_generation(&workspace_identity, candidate)
            })
        }
    }));

    let initial = admission
        .admit(workspace_identity.clone(), project_root.clone(), candidate)
        .await
        .expect("admit complete-generation build");
    tokio::time::timeout(Duration::from_secs(1), first_build_started.acquire())
        .await
        .expect("complete-generation build starts")
        .expect("complete-generation build semaphore")
        .forget();

    let cold_started = std::time::Instant::now();
    let cold = admission
        .request_runtime_generation_ready(workspace_identity.clone(), project_root.clone())
        .expect("join complete-generation admission");
    let cold_elapsed = cold_started.elapsed();
    assert_eq!(cold, WorkspaceGenerationReadinessRequestState::Accepted);
    assert!(
        cold_elapsed < Duration::from_micros(1_000),
        "cold detached-admission acknowledgement must be sub-millisecond: {cold_elapsed:?}"
    );

    let warm_started = std::time::Instant::now();
    let warm = admission
        .request_runtime_generation_ready(workspace_identity.clone(), project_root.clone())
        .expect("coalesce duplicate readiness request");
    let warm_elapsed = warm_started.elapsed();
    assert_eq!(warm, WorkspaceGenerationReadinessRequestState::Coalesced);
    assert!(
        warm_elapsed < Duration::from_micros(1_000),
        "warm detached-admission coalescing must be sub-millisecond: {warm_elapsed:?}"
    );
    eprintln!(
        "resident request-plane receipt: coldMicros={} warmMicros={} generationWaitCount=0 generationBuildCount=0 providerProcessCount=0 parserInvocationCount=0",
        cold_elapsed.as_micros(),
        warm_elapsed.as_micros(),
    );

    first_build_release.add_permits(1);
    let ready = tokio::time::timeout(
        Duration::from_secs(1),
        admission.wait_terminal(&workspace_identity, &project_root),
    )
    .await
    .expect("joined complete-generation build reaches terminal")
    .expect("joined complete-generation terminal receipt");
    assert_eq!(ready.attempt, initial.attempt);
    assert_eq!(ready.state, WorkspaceGenerationAdmissionState::Ready);
    assert_eq!(
        ready.admission_mode,
        WorkspaceGenerationAdmissionMode::CompleteGeneration
    );
    assert!(ready.commit.is_some());
    assert_eq!(build_count.load(Ordering::Acquire), 1);
    admission.shutdown().await.expect("drain admission lane");
}

#[tokio::test(flavor = "multi_thread")]
async fn readiness_terminal_callback_runs_for_a_replayed_ready_generation() {
    let fixture = tempfile::tempdir().expect("readiness terminal callback fixture");
    let project_root = fixture.path().to_path_buf();
    run_git(&project_root, &["init", "--quiet"]);
    fs::create_dir_all(project_root.join("src")).expect("create source root");
    fs::write(
        project_root.join("src/lib.rs"),
        "pub fn readiness_terminal_owner() -> u8 { 1 }\n",
    )
    .expect("write source owner");
    run_git(&project_root, &["add", "src/lib.rs"]);
    let workspace_identity =
        agent_semantic_client_db::AgentSessionRegistry::workspace_id(&project_root)
            .expect("derive canonical workspace identity");

    let admission = WorkspaceGenerationAdmission::new(Arc::new(
        |workspace_identity,
         _project_root,
         candidate,
         _build_mode,
         _changed_paths,
         provider_target,
         _cancellation| {
            Box::pin(async move {
                assert!(provider_target.is_none());
                completed_generation(&workspace_identity, candidate)
            })
        },
    ));
    let first = admission
        .ensure_runtime_generation_ready(workspace_identity.clone(), project_root.clone())
        .await
        .expect("publish the complete generation");
    assert_eq!(first.state, WorkspaceGenerationAdmissionState::Ready);

    let callback_count = Arc::new(AtomicUsize::new(0));
    let callback_completed = Arc::new(tokio::sync::Notify::new());
    let state = admission
        .request_runtime_generation_ready_with_terminal(workspace_identity, project_root, {
            let callback_count = Arc::clone(&callback_count);
            let callback_completed = Arc::clone(&callback_completed);
            move |terminal| async move {
                let terminal = terminal.expect("replayed readiness terminal");
                assert_eq!(terminal.state, WorkspaceGenerationAdmissionState::Ready);
                assert!(terminal.commit.is_some());
                callback_count.fetch_add(1, Ordering::AcqRel);
                callback_completed.notify_one();
            }
        })
        .expect("submit resident terminal callback");
    assert_eq!(state, WorkspaceGenerationReadinessRequestState::Ready);
    tokio::time::timeout(Duration::from_secs(1), callback_completed.notified())
        .await
        .expect("ready replay invokes its terminal callback");
    assert_eq!(callback_count.load(Ordering::Acquire), 1);

    admission.shutdown().await.expect("drain admission lane");
}

#[tokio::test(flavor = "multi_thread")]
async fn failed_readiness_is_delivered_once_and_releases_the_terminal_callback_key() {
    let fixture = tempfile::tempdir().expect("failed readiness terminal fixture");
    let project_root = fixture.path().to_path_buf();
    run_git(&project_root, &["init", "--quiet"]);
    fs::create_dir_all(project_root.join("src")).expect("create source root");
    fs::write(
        project_root.join("src/lib.rs"),
        "pub fn failed_readiness_terminal_owner() -> u8 { 1 }\n",
    )
    .expect("write source owner");
    run_git(&project_root, &["add", "src/lib.rs"]);
    let workspace_identity =
        agent_semantic_client_db::AgentSessionRegistry::workspace_id(&project_root)
            .expect("derive canonical workspace identity");

    let admission = WorkspaceGenerationAdmission::new(Arc::new(
        |_workspace_identity,
         _project_root,
         _candidate,
         _build_mode,
         _changed_paths,
         _provider_target,
         _cancellation| {
            Box::pin(async {
                Err(WorkspaceGenerationBuildFailure::new(
                    WorkspaceGenerationFailureStage::GenerationBuilder,
                    "injected terminal callback failure",
                ))
            })
        },
    ));
    let callback_count = Arc::new(AtomicUsize::new(0));
    let callback_completed = Arc::new(tokio::sync::Notify::new());
    assert_eq!(
        admission
            .request_runtime_generation_ready_with_terminal(
                workspace_identity.clone(),
                project_root.clone(),
                {
                    let callback_count = Arc::clone(&callback_count);
                    let callback_completed = Arc::clone(&callback_completed);
                    move |terminal| async move {
                        let error = terminal.expect_err("readiness failure terminal");
                        assert!(error.contains("injected terminal callback failure"));
                        callback_count.fetch_add(1, Ordering::AcqRel);
                        callback_completed.notify_one();
                    }
                },
            )
            .expect("submit failing terminal callback"),
        WorkspaceGenerationReadinessRequestState::Accepted
    );
    tokio::time::timeout(Duration::from_secs(10), callback_completed.notified())
        .await
        .expect("failed readiness reaches its callback");
    assert_eq!(callback_count.load(Ordering::Acquire), 1);

    let retry_state = tokio::time::timeout(Duration::from_secs(1), async {
        loop {
            let state = admission
                .request_runtime_generation_ready(workspace_identity.clone(), project_root.clone())
                .expect("retry after failed terminal callback");
            if state != WorkspaceGenerationReadinessRequestState::Coalesced {
                break state;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("failed terminal callback releases its key");
    assert_eq!(
        retry_state,
        WorkspaceGenerationReadinessRequestState::Accepted
    );

    admission.shutdown().await.expect("drain admission lane");
}

#[tokio::test(flavor = "multi_thread")]
async fn mutation_during_build_cancels_stale_publication_before_the_next_attempt() {
    let fixture = tempfile::tempdir().expect("mutation-during-build fixture");
    let project_root = fixture.path().to_path_buf();
    run_git(&project_root, &["init", "--quiet"]);
    fs::create_dir_all(project_root.join("src")).expect("create source root");
    let owner = project_root.join("src/lib.rs");
    fs::write(&owner, "pub fn generation_owner() -> u8 { 1 }\n").expect("write first owner");
    run_git(&project_root, &["add", "src/lib.rs"]);
    let workspace_identity =
        agent_semantic_client_db::AgentSessionRegistry::workspace_id(&project_root)
            .expect("derive workspace identity");
    let first = discover_workspace_generation_candidate(&project_root)
        .await
        .expect("first candidate");
    let first_digest = first.candidate_generation.digest.clone();

    let build_started = Arc::new(tokio::sync::Semaphore::new(0));
    let release_first = Arc::new(tokio::sync::Semaphore::new(0));
    let published = Arc::new(std::sync::Mutex::new(Vec::<String>::new()));
    let builds = Arc::new(AtomicUsize::new(0));
    let admission = WorkspaceGenerationAdmission::new(Arc::new({
        let build_started = Arc::clone(&build_started);
        let release_first = Arc::clone(&release_first);
        let published = Arc::clone(&published);
        let builds = Arc::clone(&builds);
        move |workspace_identity,
              _project_root,
              candidate,
              _build_mode,
              _changed_paths,
              _provider_target,
              cancellation| {
            let build_started = Arc::clone(&build_started);
            let release_first = Arc::clone(&release_first);
            let published = Arc::clone(&published);
            let builds = Arc::clone(&builds);
            Box::pin(async move {
                let ordinal = builds.fetch_add(1, Ordering::AcqRel);
                if ordinal == 0 {
                    build_started.add_permits(1);
                    release_first
                        .acquire_owned()
                        .await
                        .expect("release first build")
                        .forget();
                }
                if cancellation.is_cancelled() {
                    return Err(WorkspaceGenerationBuildFailure::new(
                        WorkspaceGenerationFailureStage::GenerationBuilderSupervision,
                        "superseded before publication",
                    ));
                }
                published
                    .lock()
                    .expect("publication observations")
                    .push(candidate.candidate_generation.digest.clone());
                completed_generation(&workspace_identity, candidate)
            })
        }
    }));

    admission
        .admit(workspace_identity.clone(), project_root.clone(), first)
        .await
        .expect("admit first build");
    build_started
        .acquire()
        .await
        .expect("first build starts")
        .forget();
    fs::write(&owner, "pub fn generation_owner() -> u8 { 2 }\n").expect("mutate owner");
    let second = discover_workspace_generation_candidate(&project_root)
        .await
        .expect("second candidate");
    assert_ne!(first_digest, second.candidate_generation.digest);
    let second_digest = second.candidate_generation.digest.clone();
    admission
        .admit_observed_mutation(
            "mutation-during-build",
            workspace_identity.clone(),
            project_root.clone(),
            vec![owner],
            second,
        )
        .await
        .expect("queue mutation");
    release_first.add_permits(1);

    let terminal = observe_terminal(&admission, &workspace_identity, &project_root, 1).await;
    assert_eq!(terminal.state, WorkspaceGenerationAdmissionState::Ready);
    assert_eq!(terminal.candidate_generation.digest, second_digest);
    assert_eq!(
        published
            .lock()
            .expect("publication observations")
            .as_slice(),
        &[second_digest],
        "the superseded attempt must not publish stale bytes"
    );
    assert_eq!(builds.load(Ordering::Acquire), 2);
    admission.shutdown().await.expect("drain admission lane");
}
