use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use crate::runtime_server_admission::{
    WorkspaceGenerationAdmission, WorkspaceGenerationAdmissionMode,
    WorkspaceGenerationAdmissionReceipt, WorkspaceGenerationAdmissionState,
    WorkspaceGenerationAdmissionTrigger, WorkspaceGenerationBuildCompletion,
    WorkspaceGenerationBuildFailure, WorkspaceGenerationBuildMode,
    WorkspaceGenerationCandidateIdentity, WorkspaceGenerationCommitReceipt,
    WorkspaceGenerationFailureStage, record_workspace_generation_candidate,
};

fn candidate() -> WorkspaceGenerationCandidateIdentity {
    WorkspaceGenerationCandidateIdentity {
        candidate_generation: agent_semantic_runtime::git::RepositoryCandidateGeneration {
            algorithm: "blake3-worktree-state-v1".to_owned(),
            digest: format!("blake3:{:064x}", 71),
            authorities: vec![agent_semantic_runtime::git::RepositoryCandidateAuthority::GitIndex],
        },
        policy_overlay_digest: format!("blake3:{:064x}", 72),
    }
}

fn completed(
    candidate: WorkspaceGenerationCandidateIdentity,
) -> Result<WorkspaceGenerationBuildCompletion, WorkspaceGenerationBuildFailure> {
    WorkspaceGenerationBuildCompletion::new(
        candidate,
        WorkspaceGenerationCommitReceipt {
            projection_capability: crate::fixture::ready_projection_capability_fixture(
                "workspace-query-demand",
                format!("blake3-256:{:064x}", 73),
                format!("blake3-256:{:064x}", 74),
                1,
            ),
            active_epoch: 1,
            generation_digest: format!("blake3-256:{:064x}", 73),
            source_root_digest: format!("blake3-256:{:064x}", 74),
        },
    )
    .map_err(|error| {
        WorkspaceGenerationBuildFailure::new(
            WorkspaceGenerationFailureStage::CanonicalGenerationPublication,
            error,
        )
    })
}

fn root(label: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!("asp-query-demand-{label}-{}", std::process::id()))
}

async fn wait_for_attempt(
    admission: &WorkspaceGenerationAdmission,
    workspace_identity: &str,
    project_root: &std::path::Path,
    attempt: u64,
) -> WorkspaceGenerationAdmissionReceipt {
    tokio::time::timeout(std::time::Duration::from_secs(5), async {
        loop {
            if let Some(receipt) = admission.current(workspace_identity, project_root)
                && receipt.attempt == attempt
            {
                return receipt;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("query-demand attempt publication")
}

async fn wait_for_build_count(builds: &AtomicUsize, expected: usize) {
    tokio::time::timeout(std::time::Duration::from_secs(5), async {
        while builds.load(Ordering::Acquire) != expected {
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("query-demand build start");
}

#[tokio::test]
async fn missing_runtime_admission_authority_is_not_misreported_as_in_progress() {
    let registry = crate::runtime_server_workspace::RuntimeServerWorkspaceRegistry::new(root(
        "authority-unavailable-runtime",
    ))
    .expect("create runtime workspace registry");
    let error = crate::workspace_db_ipc_server::require_or_submit_terminal_generation_for_read(
        &registry,
        None,
        "workspace-without-runtime-admission",
        &root("authority-unavailable"),
        Vec::new(),
    )
    .await
    .expect_err("missing Runtime admission authority fails terminally");
    assert!(error.contains("reasonKind=runtime-generation-admission-unavailable"));
    assert!(!error.contains("active-workspace-generation-required"));
}

#[test]
fn exact_selector_query_demand_is_parser_targeted_to_its_owner() {
    let project_root = root("selector-target");
    let operation = crate::workspace_db_ipc::WorkspaceDbIpcOperation::ReadRuntimeSelector {
        project_root: project_root.display().to_string(),
        language_id: agent_semantic_client_core::LanguageId::from("rust"),
        projection_kind:
            crate::runtime_server_workspace::ExactProjectionKind::CallableSkeleton,
        structural_selector: "rust://crates/agent-semantic-client-db/src/runtime_server_admission.rs#item/method/admit/scope/implementation-owner/type/WorkspaceGenerationAdmission".to_owned(),
    };
    assert_eq!(
        crate::workspace_db_ipc_server::resident_read_query_targets_for_test(
            &operation,
            &project_root,
        )
        .expect("canonical selector target"),
        [project_root.join("crates/agent-semantic-client-db/src/runtime_server_admission.rs")]
    );
}

#[test]
fn provider_search_query_demand_is_targeted_to_its_language_provider() {
    let operation = crate::workspace_db_ipc::WorkspaceDbIpcOperation::ProviderSearch {
        operation_id: "query-demand-provider-target".to_owned(),
        project_root: root("provider-target").display().to_string(),
        language_id: agent_semantic_client_core::LanguageId::from("rust"),
        args: vec!["pipe".to_owned(), "config fixture".to_owned()],
    };

    assert_eq!(
        crate::workspace_db_ipc_server::resident_read_query_provider_target_for_test(&operation),
        Some(
            crate::runtime_server_admission::WorkspaceGenerationProviderTarget {
                language_id: "rust".to_owned(),
                provider_id: None,
            }
        )
    );
}

#[tokio::test]
async fn provider_targeted_query_demand_keeps_language_scoped_coverage() {
    let observed = Arc::new(std::sync::Mutex::new(None));
    let admission = WorkspaceGenerationAdmission::new(Arc::new({
        let observed = Arc::clone(&observed);
        move |_, _, candidate, _, target_paths, provider_target, _| {
            let observed = Arc::clone(&observed);
            Box::pin(async move {
                *observed.lock().expect("provider target observation lock") =
                    Some((target_paths.as_ref().clone(), provider_target));
                completed(candidate)
            })
        }
    }));
    let project_root = root("provider-targeted-coverage");
    let workspace_identity = "workspace-query-demand-provider-target";
    record_workspace_generation_candidate(project_root.clone(), candidate())
        .expect("record query-demand candidate");

    assert!(
        admission
            .submit_query_demand_with_provider(
                workspace_identity.to_owned(),
                project_root.clone(),
                Vec::new(),
                Some(
                    crate::runtime_server_admission::WorkspaceGenerationProviderTarget {
                        language_id: "rust".to_owned(),
                        provider_id: None,
                    }
                ),
            )
            .await
            .expect("submit provider-targeted query demand")
    );
    let ready = admission
        .wait_terminal(workspace_identity, &project_root)
        .await
        .expect("provider-targeted query demand completes");
    assert_eq!(ready.state, WorkspaceGenerationAdmissionState::Ready);
    let (coverage, provider_target) = observed
        .lock()
        .expect("provider target observation lock")
        .clone()
        .expect("provider-targeted builder invocation");
    assert_eq!(
        coverage,
        std::collections::BTreeSet::from([project_root
            .join(".asp-runtime-query-demand")
            .join("provider")
            .join("rust")])
    );
    assert_eq!(
        provider_target,
        Some(
            crate::runtime_server_admission::WorkspaceGenerationProviderTarget {
                language_id: "rust".to_owned(),
                provider_id: None,
            }
        )
    );
    admission.shutdown().await.expect("shutdown admission");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn missing_ipc_read_guard_submits_runtime_owned_cold_admission() {
    let builds = Arc::new(AtomicUsize::new(0));
    let observed_targets = Arc::new(std::sync::Mutex::new(Vec::new()));
    let started = Arc::new(tokio::sync::Notify::new());
    let release = Arc::new(tokio::sync::Notify::new());
    let admission = Arc::new(WorkspaceGenerationAdmission::new(Arc::new({
        let builds = Arc::clone(&builds);
        let observed_targets = Arc::clone(&observed_targets);
        let started = Arc::clone(&started);
        let release = Arc::clone(&release);
        move |_, _, candidate, _, target_paths, _provider_target, _| {
            let builds = Arc::clone(&builds);
            let observed_targets = Arc::clone(&observed_targets);
            let started = Arc::clone(&started);
            let release = Arc::clone(&release);
            Box::pin(async move {
                builds.fetch_add(1, Ordering::AcqRel);
                observed_targets
                    .lock()
                    .expect("target observation lock")
                    .push(target_paths.as_ref().clone());
                started.notify_one();
                release.notified().await;
                completed(candidate)
            })
        }
    })));
    let registry = crate::runtime_server_workspace::RuntimeServerWorkspaceRegistry::new(root(
        "ipc-missing-runtime",
    ))
    .expect("create runtime workspace registry");
    let project_root = root("ipc-missing");
    let workspace_identity = "workspace-query-demand-ipc-missing";
    record_workspace_generation_candidate(project_root.clone(), candidate())
        .expect("record IPC cold candidate");
    let cold_target = project_root.join("src/lib.rs");

    let in_progress =
        crate::workspace_db_ipc_server::require_or_submit_terminal_generation_for_read(
            &registry,
            Some(&admission),
            workspace_identity,
            &project_root,
            vec![cold_target.clone()],
        )
        .await
        .expect_err("missing generation returns typed in-progress cause");
    assert!(in_progress.contains("reasonKind=active-workspace-generation-required"));
    assert!(in_progress.contains("enqueued query-demand cold admission"));
    started.notified().await;
    assert_eq!(builds.load(Ordering::Acquire), 1);
    assert_eq!(
        observed_targets
            .lock()
            .expect("target observation lock")
            .as_slice(),
        [std::collections::BTreeSet::from([cold_target])]
    );

    let joined = crate::workspace_db_ipc_server::require_or_submit_terminal_generation_for_read(
        &registry,
        Some(&admission),
        workspace_identity,
        &project_root,
        Vec::new(),
    )
    .await
    .expect_err("concurrent query joins Building admission");
    assert!(joined.contains("state=Building"));
    assert_eq!(builds.load(Ordering::Acquire), 1);

    release.notify_one();
    let ready = admission
        .wait_terminal(workspace_identity, &project_root)
        .await
        .expect("IPC cold admission Ready");
    assert_eq!(ready.state, WorkspaceGenerationAdmissionState::Ready);
    let unpublished =
        crate::workspace_db_ipc_server::require_or_submit_terminal_generation_for_read(
            &registry,
            Some(&admission),
            workspace_identity,
            &project_root,
            Vec::new(),
        )
        .await
        .expect_err("Ready admission without resident publication remains unreadable");
    assert!(unpublished.contains("reasonKind=active-workspace-generation-required"));
    assert!(unpublished.contains("state=Ready"));
    assert_eq!(builds.load(Ordering::Acquire), 1);
    admission.shutdown().await.expect("shutdown admission");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn missing_concurrent_queries_share_one_runtime_owned_admission_and_ready_is_reused() {
    const QUERY_COUNT: usize = 64;
    let builds = Arc::new(AtomicUsize::new(0));
    let started = Arc::new(tokio::sync::Notify::new());
    let release = Arc::new(tokio::sync::Notify::new());
    let admission = Arc::new(WorkspaceGenerationAdmission::new(Arc::new({
        let builds = Arc::clone(&builds);
        let started = Arc::clone(&started);
        let release = Arc::clone(&release);
        move |_, _, candidate, _, _, _provider_target, _| {
            let builds = Arc::clone(&builds);
            let started = Arc::clone(&started);
            let release = Arc::clone(&release);
            Box::pin(async move {
                builds.fetch_add(1, Ordering::AcqRel);
                started.notify_one();
                release.notified().await;
                completed(candidate)
            })
        }
    })));
    let project_root = root("single-flight");
    let workspace_identity = "workspace-query-demand-single-flight";
    record_workspace_generation_candidate(project_root.clone(), candidate())
        .expect("record candidate");

    let mut requests = tokio::task::JoinSet::new();
    for _ in 0..QUERY_COUNT {
        let admission = Arc::clone(&admission);
        let project_root = project_root.clone();
        requests.spawn(async move {
            admission
                .submit_query_demand(workspace_identity.to_owned(), project_root, Vec::new())
                .await
        });
    }
    let mut enqueued = 0;
    while let Some(result) =
        tokio::time::timeout(std::time::Duration::from_secs(2), requests.join_next())
            .await
            .expect("query-demand reservation reaches a terminal response")
    {
        enqueued += usize::from(result.expect("query request").expect("submit query demand"));
    }
    assert_eq!(enqueued, 1);
    tokio::time::timeout(std::time::Duration::from_secs(2), started.notified())
        .await
        .expect("shared admission builder starts");
    assert_eq!(builds.load(Ordering::Acquire), 1);

    // Dropping every initiating request leaves the Runtime-owned build alive.
    release.notify_one();
    let ready = tokio::time::timeout(
        std::time::Duration::from_secs(2),
        admission.wait_terminal(workspace_identity, &project_root),
    )
    .await
    .expect("shared admission reaches a terminal receipt")
    .expect("query-demand Ready receipt");
    assert_eq!(ready.state, WorkspaceGenerationAdmissionState::Ready);
    assert_eq!(
        ready.trigger,
        WorkspaceGenerationAdmissionTrigger::QueryDemand
    );
    assert_eq!(
        ready.admission_mode,
        WorkspaceGenerationAdmissionMode::ColdTargeted
    );
    assert!(ready.request_lifetime_independent);

    assert!(
        !admission
            .submit_query_demand(
                workspace_identity.to_owned(),
                project_root.clone(),
                Vec::new()
            )
            .await
            .expect("reuse Ready query demand")
    );
    tokio::task::yield_now().await;
    assert_eq!(builds.load(Ordering::Acquire), 1);
    assert!(
        !project_root.exists(),
        "warm admission lookup must not open or materialize a client DB"
    );
    admission.shutdown().await.expect("shutdown admission");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn ready_generation_rebuilds_for_an_uncovered_cold_target() {
    let observed = Arc::new(std::sync::Mutex::new(Vec::new()));
    let admission = WorkspaceGenerationAdmission::new(Arc::new({
        let observed = Arc::clone(&observed);
        move |_, _, candidate, mode, target_paths, _provider_target, _| {
            let observed = Arc::clone(&observed);
            Box::pin(async move {
                observed
                    .lock()
                    .expect("target coverage observation lock")
                    .push((mode, target_paths.as_ref().clone()));
                completed(candidate)
            })
        }
    }));
    let project_root = root("cross-target");
    let workspace_identity = "workspace-query-demand-cross-target";
    let target_a = project_root.join("src/a.rs");
    let target_b = project_root.join("src/b.rs");
    record_workspace_generation_candidate(project_root.clone(), candidate())
        .expect("record cross-target candidate");

    assert!(
        admission
            .submit_query_demand(
                workspace_identity.to_owned(),
                project_root.clone(),
                vec![target_a.clone()],
            )
            .await
            .expect("submit first cold target")
    );
    let _ = wait_for_attempt(&admission, workspace_identity, &project_root, 1).await;
    let first = admission
        .wait_terminal_attempt(workspace_identity, &project_root, 1)
        .await
        .expect("first target Ready");
    assert_eq!(first.state, WorkspaceGenerationAdmissionState::Ready);
    record_workspace_generation_candidate(project_root.clone(), candidate())
        .expect("record uncovered-target candidate");
    assert!(
        admission
            .submit_query_demand(
                workspace_identity.to_owned(),
                project_root.clone(),
                vec![target_b.clone()],
            )
            .await
            .expect("submit uncovered cold target")
    );
    let _ = wait_for_attempt(&admission, workspace_identity, &project_root, 2).await;
    let second = admission
        .wait_terminal_attempt(workspace_identity, &project_root, 2)
        .await
        .expect("second target Ready");
    assert_eq!(second.state, WorkspaceGenerationAdmissionState::Ready);
    assert_eq!(
        observed.lock().expect("target observations").as_slice(),
        [
            (
                WorkspaceGenerationBuildMode::RestoreOrBuild,
                std::collections::BTreeSet::from([target_a]),
            ),
            (
                WorkspaceGenerationBuildMode::RebuildAfterMutation,
                std::collections::BTreeSet::from([target_b]),
            ),
        ]
    );
    assert!(
        !admission
            .submit_query_demand(
                workspace_identity.to_owned(),
                project_root.clone(),
                vec![project_root.join("src/a.rs"), project_root.join("src/b.rs")],
            )
            .await
            .expect("reuse combined Ready coverage")
    );
    admission.shutdown().await.expect("shutdown admission");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn uncovered_target_waits_for_the_active_build_then_gets_its_own_admission() {
    let builds = Arc::new(AtomicUsize::new(0));
    let observed = Arc::new(std::sync::Mutex::new(Vec::new()));
    let releases = Arc::new(tokio::sync::Semaphore::new(0));
    let admission = WorkspaceGenerationAdmission::new(Arc::new({
        let builds = Arc::clone(&builds);
        let observed = Arc::clone(&observed);
        let releases = Arc::clone(&releases);
        move |_, _, candidate, mode, target_paths, _provider_target, _| {
            let builds = Arc::clone(&builds);
            let observed = Arc::clone(&observed);
            let releases = Arc::clone(&releases);
            Box::pin(async move {
                builds.fetch_add(1, Ordering::AcqRel);
                observed
                    .lock()
                    .expect("target observation lock")
                    .push((mode, target_paths.as_ref().clone()));
                releases
                    .acquire()
                    .await
                    .expect("Runtime build release")
                    .forget();
                completed(candidate)
            })
        }
    }));
    let project_root = root("concurrent-cross-target");
    let workspace_identity = "workspace-query-demand-concurrent-cross-target";
    let target_a = project_root.join("src/a.rs");
    let target_b = project_root.join("src/b.rs");
    record_workspace_generation_candidate(project_root.clone(), candidate())
        .expect("record first target candidate");

    assert!(
        admission
            .submit_query_demand(
                workspace_identity.to_owned(),
                project_root.clone(),
                vec![target_a.clone()],
            )
            .await
            .expect("submit first target")
    );
    let _ = wait_for_attempt(&admission, workspace_identity, &project_root, 1).await;
    wait_for_build_count(&builds, 1).await;
    assert_eq!(builds.load(Ordering::Acquire), 1);

    assert!(
        admission
            .submit_query_demand(
                workspace_identity.to_owned(),
                project_root.clone(),
                vec![target_b.clone()],
            )
            .await
            .expect("submit uncovered target while first build is active")
    );
    tokio::task::yield_now().await;
    assert_eq!(
        builds.load(Ordering::Acquire),
        1,
        "uncovered target must wait instead of racing the active generation"
    );
    record_workspace_generation_candidate(project_root.clone(), candidate())
        .expect("record second target candidate");
    releases.add_permits(1);
    let _ = wait_for_attempt(&admission, workspace_identity, &project_root, 2).await;
    wait_for_build_count(&builds, 2).await;
    assert_eq!(builds.load(Ordering::Acquire), 2);
    releases.add_permits(1);
    let ready = admission
        .wait_terminal_attempt(workspace_identity, &project_root, 2)
        .await
        .expect("uncovered target Ready");
    assert_eq!(ready.state, WorkspaceGenerationAdmissionState::Ready);
    assert_eq!(
        observed.lock().expect("target observations").as_slice(),
        [
            (
                WorkspaceGenerationBuildMode::RestoreOrBuild,
                std::collections::BTreeSet::from([target_a]),
            ),
            (
                WorkspaceGenerationBuildMode::RebuildAfterMutation,
                std::collections::BTreeSet::from([target_b]),
            ),
        ]
    );
    admission.shutdown().await.expect("shutdown admission");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn failed_query_demand_releases_runtime_admission_for_retry() {
    let builds = Arc::new(AtomicUsize::new(0));
    let admission = WorkspaceGenerationAdmission::new(Arc::new({
        let builds = Arc::clone(&builds);
        move |_, _, candidate, _, _, _provider_target, _| {
            let attempt = builds.fetch_add(1, Ordering::AcqRel) + 1;
            Box::pin(async move {
                if attempt == 1 {
                    Err(WorkspaceGenerationBuildFailure::new(
                        WorkspaceGenerationFailureStage::SourceBuilder,
                        "synthetic first query-demand failure",
                    ))
                } else {
                    completed(candidate)
                }
            })
        }
    }));
    let project_root = root("retry");
    let workspace_identity = "workspace-query-demand-retry";

    record_workspace_generation_candidate(project_root.clone(), candidate())
        .expect("record first candidate");
    assert!(
        admission
            .submit_query_demand(
                workspace_identity.to_owned(),
                project_root.clone(),
                Vec::new()
            )
            .await
            .expect("submit first query demand")
    );
    let _ = wait_for_attempt(&admission, workspace_identity, &project_root, 1).await;
    let failed = admission
        .wait_terminal_attempt(workspace_identity, &project_root, 1)
        .await
        .expect("first query demand terminal");
    assert_eq!(failed.state, WorkspaceGenerationAdmissionState::Failed);

    record_workspace_generation_candidate(project_root.clone(), candidate())
        .expect("record retry candidate");
    assert!(
        admission
            .submit_query_demand(
                workspace_identity.to_owned(),
                project_root.clone(),
                Vec::new()
            )
            .await
            .expect("retry query demand")
    );
    let _ = wait_for_attempt(&admission, workspace_identity, &project_root, 2).await;
    let ready = admission
        .wait_terminal_attempt(workspace_identity, &project_root, 2)
        .await
        .expect("retry query demand terminal");
    assert_eq!(ready.state, WorkspaceGenerationAdmissionState::Ready);
    assert_eq!(builds.load(Ordering::Acquire), 2);
    admission.shutdown().await.expect("shutdown admission");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn runtime_shutdown_is_the_query_demand_cancellation_authority() {
    let started = Arc::new(tokio::sync::Notify::new());
    let admission = WorkspaceGenerationAdmission::new(Arc::new({
        let started = Arc::clone(&started);
        move |_, _, _, _, _, _provider_target, cancellation| {
            let started = Arc::clone(&started);
            Box::pin(async move {
                started.notify_one();
                cancellation.cancelled().await;
                Err(WorkspaceGenerationBuildFailure::new(
                    WorkspaceGenerationFailureStage::GenerationBuilder,
                    "Runtime shutdown cancelled query-demand build",
                ))
            })
        }
    }));
    let project_root = root("shutdown");
    let workspace_identity = "workspace-query-demand-shutdown";
    record_workspace_generation_candidate(project_root.clone(), candidate())
        .expect("record shutdown candidate");
    assert!(
        admission
            .submit_query_demand(
                workspace_identity.to_owned(),
                project_root.clone(),
                Vec::new()
            )
            .await
            .expect("submit shutdown query demand")
    );
    started.notified().await;

    admission.shutdown().await.expect("Runtime shutdown");
    let cancelled = admission
        .current(workspace_identity, &project_root)
        .expect("cancelled query-demand receipt");
    assert_eq!(
        cancelled.state,
        WorkspaceGenerationAdmissionState::Cancelled
    );
    assert_eq!(cancelled.cancellation_authority, "runtime-server");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn artifact_publication_admission_builds_one_full_ready_generation() {
    let build_count = Arc::new(AtomicUsize::new(0));
    let observed_target = Arc::new(std::sync::Mutex::new(None));
    let admission = WorkspaceGenerationAdmission::new(Arc::new({
        let build_count = Arc::clone(&build_count);
        let observed_target = Arc::clone(&observed_target);
        move |_, _, candidate, _, _, provider_target, _| {
            let build_count = Arc::clone(&build_count);
            let observed_target = Arc::clone(&observed_target);
            Box::pin(async move {
                build_count.fetch_add(1, Ordering::SeqCst);
                *observed_target
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner) = provider_target;
                completed(candidate)
            })
        }
    }));
    let project_root = root("artifact-publication");
    let workspace_identity = "workspace-artifact-publication";
    record_workspace_generation_candidate(project_root.clone(), candidate())
        .expect("record artifact publication candidate");

    let receipt = admission
        .admit_artifact_publication_and_wait(
            workspace_identity.to_owned(),
            project_root,
            "gerbil-scheme".to_owned(),
            "asp-gerbil-scheme".to_owned(),
        )
        .await
        .expect("artifact publication terminal receipt");

    assert_eq!(receipt.state, WorkspaceGenerationAdmissionState::Ready);
    assert_eq!(
        receipt.trigger,
        WorkspaceGenerationAdmissionTrigger::ArtifactPublication
    );
    assert_eq!(
        receipt.admission_mode,
        WorkspaceGenerationAdmissionMode::FullRecovery
    );
    assert!(receipt.commit.is_some());
    assert_eq!(build_count.load(Ordering::SeqCst), 1);
    let observed_target = observed_target
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .clone()
        .expect("artifact publication must retain its explicit provider target");
    assert_eq!(observed_target.language_id, "gerbil-scheme");
    assert_eq!(
        observed_target.provider_id.as_deref(),
        Some("asp-gerbil-scheme")
    );
}
