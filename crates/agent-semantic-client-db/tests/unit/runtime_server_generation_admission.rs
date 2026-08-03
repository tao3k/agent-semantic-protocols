use std::sync::Arc;

use agent_semantic_client_db::runtime_server_admission::{
    WorkspaceGenerationAdmission, WorkspaceGenerationAdmissionReceipt,
    WorkspaceGenerationAdmissionState, WorkspaceGenerationMutationAdmissionReceipt,
    WORKSPACE_GENERATION_ADMISSION_RECEIPT_SCHEMA_ID,
    WORKSPACE_GENERATION_MUTATION_ADMISSION_RECEIPT_SCHEMA_ID,
};
use agent_semantic_client_db::runtime_server_admission_catalog::{
    RuntimeWorkspaceAdmissionCatalog, RuntimeWorkspaceAdmissionCatalogEntry,
};
use tokio::sync::{Barrier, Mutex};

fn ready_receipt(workspace_identity: &str) -> WorkspaceGenerationAdmissionReceipt {
    WorkspaceGenerationAdmissionReceipt {
        schema_id: WORKSPACE_GENERATION_ADMISSION_RECEIPT_SCHEMA_ID.to_owned(),
        schema_version: "1".to_owned(),
        workspace_identity: workspace_identity.to_owned(),
        state: WorkspaceGenerationAdmissionState::Ready,
        accepted: true,
        attempt: 1,
        commit: Some(
            agent_semantic_client_db::runtime_server_admission::WorkspaceGenerationCommitReceipt {
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

fn committed_generation(
) -> agent_semantic_client_db::runtime_server_admission::WorkspaceGenerationCommitReceipt {
    agent_semantic_client_db::runtime_server_admission::WorkspaceGenerationCommitReceipt {
        active_epoch: 1,
        generation_digest:
            "blake3-256:1111111111111111111111111111111111111111111111111111111111111111".to_owned(),
        source_root_digest:
            "blake3-256:2222222222222222222222222222222222222222222222222222222222222222".to_owned(),
    }
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

#[test]
fn mutation_receipt_rejects_count_drift_and_duplicate_workspace_identity() {
    let count_drift = WorkspaceGenerationMutationAdmissionReceipt {
        schema_id: WORKSPACE_GENERATION_MUTATION_ADMISSION_RECEIPT_SCHEMA_ID.to_owned(),
        schema_version: "1".to_owned(),
        mutation_id: "mutation-count-drift".to_owned(),
        changed_path_count: 2,
        affected_workspace_count: 1,
        receipts: vec![
            ready_receipt("workspace-parent"),
            ready_receipt("workspace-nested"),
        ],
    };
    assert_eq!(
        count_drift.validate().expect_err("count drift must fail"),
        "workspace mutation admission receipt count does not match receipts"
    );

    let duplicate = WorkspaceGenerationMutationAdmissionReceipt {
        schema_id: WORKSPACE_GENERATION_MUTATION_ADMISSION_RECEIPT_SCHEMA_ID.to_owned(),
        schema_version: "1".to_owned(),
        mutation_id: "mutation-duplicate-workspace".to_owned(),
        changed_path_count: 2,
        affected_workspace_count: 2,
        receipts: vec![
            ready_receipt("workspace-parent"),
            ready_receipt("workspace-parent"),
        ],
    };
    assert_eq!(
        duplicate
            .validate()
            .expect_err("duplicate workspace identity must fail"),
        "workspace mutation admission receipt repeats a workspace identity"
    );
}

#[tokio::test]
async fn mutation_admission_rejects_non_normalized_paths_and_workspace_identity_drift() {
    let temp = tempfile::tempdir().expect("temporary resident catalog");
    let parent_root = temp.path().join("repository");
    let conflicting_root = temp.path().join("other-checkout");
    let catalog = RuntimeWorkspaceAdmissionCatalog::load(temp.path().join("catalog.json"))
        .await
        .expect("load resident workspace catalog");
    catalog
        .record(RuntimeWorkspaceAdmissionCatalogEntry {
            workspace_identity: "workspace-parent".to_owned(),
            project_root: parent_root.clone(),
        })
        .await
        .expect("record parent workspace");
    let admission = WorkspaceGenerationAdmission::new(Arc::new(|_, _, _| {
        Box::pin(async { panic!("invalid mutation admission must not start a generation build") })
    }))
    .with_catalog(catalog);

    let path_error = admission
        .admit_changed_paths(
            "mutation-path-normalization",
            "workspace-parent",
            parent_root.clone(),
            vec![std::path::PathBuf::from("src/../src/lib.rs")],
        )
        .await
        .expect_err("parent directory component must fail closed");
    assert!(path_error.contains("path must be normalized"));

    let identity_error = admission
        .admit_changed_paths(
            "mutation-identity-drift",
            "workspace-parent",
            conflicting_root.clone(),
            vec![conflicting_root.join("src/lib.rs")],
        )
        .await
        .expect_err("workspace identity drift must fail closed");
    assert!(identity_error.contains("identity already owns a different resident root"));
}

#[test]
fn cold_restore_publishes_committed_generation_without_live_checkout_probe() {
    let source = include_str!("../../src/runtime_server.rs");
    assert!(source.contains("canonical_materialization_matches_admitted_generation"));
    assert!(!source.contains("canonical_materialization_matches_candidate_generation"));
    assert!(!source.contains("discover_repository_candidate_snapshot"));
    assert!(!source.contains("if materialization.project_resolutions.is_empty()"));
}

#[tokio::test(flavor = "multi_thread")]
async fn concurrent_workspace_admission_is_single_flight() {
    const REQUEST_COUNT: usize = 256;
    let build_count = Arc::new(Mutex::new(0_u32));
    let release = Arc::new(Barrier::new(2));
    let admission = WorkspaceGenerationAdmission::new(Arc::new({
        let build_count = Arc::clone(&build_count);
        let release = Arc::clone(&release);
        move |_workspace_identity, _project_root, _build_mode| {
            let build_count = Arc::clone(&build_count);
            let release = Arc::clone(&release);
            Box::pin(async move {
                *build_count.lock().await += 1;
                release.wait().await;
                Ok(committed_generation())
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
                .admit("workspace-single-flight", project_root)
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
        assert_eq!(receipt.state, WorkspaceGenerationAdmissionState::Building);
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
        .wait_terminal("workspace-single-flight", &project_root)
        .await
        .expect("wait for completed admission");
    assert_eq!(receipt.state, WorkspaceGenerationAdmissionState::Ready);
    admission
        .shutdown()
        .await
        .expect("drain generation admission lane");
}

#[tokio::test]
async fn project_roots_have_independent_admission_flights() {
    let roots = Arc::new(Mutex::new(Vec::new()));
    let admission = WorkspaceGenerationAdmission::new(Arc::new({
        let roots = Arc::clone(&roots);
        move |_workspace_identity, project_root, _build_mode| {
            let roots = Arc::clone(&roots);
            Box::pin(async move {
                roots.lock().await.push(project_root);
                Ok(committed_generation())
            })
        }
    }));
    let first_root = std::env::temp_dir().join("asp-generation-admission-first-project");
    let second_root = std::env::temp_dir().join("asp-generation-admission-second-project");

    let first = admission
        .admit("shared-repository-identity", first_root.clone())
        .await
        .expect("admit first project root");
    let second = admission
        .admit("shared-repository-identity", second_root.clone())
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

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn changed_paths_are_admitted_by_longest_resident_workspace_root() {
    let temp = tempfile::tempdir().expect("temporary resident catalog");
    let parent_root = temp.path().join("repository");
    let nested_root = parent_root.join("languages/rust-lang-project-harness");
    tokio::fs::create_dir_all(&nested_root)
        .await
        .expect("nested workspace root");
    let catalog = RuntimeWorkspaceAdmissionCatalog::load(temp.path().join("catalog.json"))
        .await
        .expect("load resident workspace catalog");
    catalog
        .record(RuntimeWorkspaceAdmissionCatalogEntry {
            workspace_identity: "workspace-parent".to_owned(),
            project_root: parent_root.clone(),
        })
        .await
        .expect("record parent workspace");
    catalog
        .record(RuntimeWorkspaceAdmissionCatalogEntry {
            workspace_identity: "workspace-nested".to_owned(),
            project_root: nested_root.clone(),
        })
        .await
        .expect("record nested workspace");

    let builds = Arc::new(Mutex::new(Vec::new()));
    let admission = WorkspaceGenerationAdmission::new(Arc::new({
        let builds = Arc::clone(&builds);
        move |workspace_identity, project_root, _build_mode| {
            let builds = Arc::clone(&builds);
            Box::pin(async move {
                builds.lock().await.push((workspace_identity, project_root));
                Ok(committed_generation())
            })
        }
    }))
    .with_catalog(catalog);

    let receipt = admission
        .admit_changed_paths(
            "mutation-parent-and-nested",
            "workspace-parent",
            parent_root.clone(),
            vec![
                parent_root.join("README.md"),
                nested_root.join("src/exact_source.rs"),
            ],
        )
        .await
        .expect("admit changed paths");
    receipt.validate().expect("valid mutation receipt");
    assert_eq!(receipt.changed_path_count, 2);
    assert_eq!(receipt.affected_workspace_count, 2);
    assert_eq!(
        receipt
            .receipts
            .iter()
            .map(|receipt| receipt.workspace_identity.as_str())
            .collect::<Vec<_>>(),
        vec!["workspace-nested", "workspace-parent"]
    );

    admission
        .wait_terminal("workspace-parent", &parent_root)
        .await
        .expect("parent workspace terminal state");
    admission
        .wait_terminal("workspace-nested", &nested_root)
        .await
        .expect("nested workspace terminal state");
    let mut builds = builds.lock().await.clone();
    builds.sort();
    assert_eq!(
        builds,
        vec![
            ("workspace-nested".to_owned(), nested_root),
            ("workspace-parent".to_owned(), parent_root),
        ]
    );

    admission
        .shutdown()
        .await
        .expect("drain changed-path admission lanes");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn distinct_mutation_queued_during_build_runs_as_the_next_generation_attempt() {
    let build_count = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let release = Arc::new(tokio::sync::Semaphore::new(0));
    let admission = WorkspaceGenerationAdmission::new(Arc::new({
        let build_count = Arc::clone(&build_count);
        let release = Arc::clone(&release);
        move |_, _, _| {
            let build_count = Arc::clone(&build_count);
            let release = Arc::clone(&release);
            Box::pin(async move {
                build_count.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                release
                    .acquire_owned()
                    .await
                    .map_err(|_| "mutation queue release closed".to_owned())?
                    .forget();
                Ok(committed_generation())
            })
        }
    }));
    let root = std::env::temp_dir().join("asp-mutation-queue-workspace");
    let changed_path = root.join("src/lib.rs");

    let first = admission
        .admit_changed_paths(
            "mutation-1",
            "workspace-mutation-queue",
            root.clone(),
            vec![changed_path.clone()],
        )
        .await
        .expect("admit first mutation");
    assert_eq!(first.receipts[0].attempt, 1);
    assert!(first.receipts[0].accepted);

    let second = admission
        .admit_changed_paths(
            "mutation-2",
            "workspace-mutation-queue",
            root.clone(),
            vec![changed_path.clone()],
        )
        .await
        .expect("queue successor mutation");
    assert_eq!(second.receipts[0].attempt, 2);
    assert!(second.receipts[0].accepted);

    let third = admission
        .admit_changed_paths(
            "mutation-3",
            "workspace-mutation-queue",
            root.clone(),
            vec![changed_path.clone()],
        )
        .await
        .expect("queue second successor mutation");
    assert_eq!(third.receipts[0].attempt, 3);
    assert!(third.receipts[0].accepted);

    let duplicate = admission
        .admit_changed_paths(
            "mutation-2",
            "workspace-mutation-queue",
            root.clone(),
            vec![changed_path.clone()],
        )
        .await
        .expect("observe queued mutation");
    assert_eq!(duplicate.receipts[0].attempt, 2);
    assert!(!duplicate.receipts[0].accepted);

    release.add_permits(1);
    tokio::time::timeout(std::time::Duration::from_millis(100), async {
        while build_count.load(std::sync::atomic::Ordering::Relaxed) < 2 {
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("successor generation attempt must start");
    release.add_permits(1);
    tokio::time::timeout(std::time::Duration::from_millis(100), async {
        while build_count.load(std::sync::atomic::Ordering::Relaxed) < 3 {
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("second successor generation attempt must start");
    release.add_permits(1);
    let terminal = tokio::time::timeout(
        std::time::Duration::from_millis(100),
        admission.wait_terminal("workspace-mutation-queue", &root),
    )
    .await
    .expect("queued successor must reach terminal state within 100ms")
    .expect("queued successor must produce a terminal receipt");
    assert_eq!(terminal.state, WorkspaceGenerationAdmissionState::Ready);
    assert_eq!(terminal.attempt, 3);
    assert_eq!(build_count.load(std::sync::atomic::Ordering::Relaxed), 3);

    let completed_duplicate = admission
        .admit_changed_paths(
            "mutation-3",
            "workspace-mutation-queue",
            root.clone(),
            vec![changed_path],
        )
        .await
        .expect("coalesce duplicate of the completed mutation");
    assert_eq!(completed_duplicate.receipts[0].attempt, 3);
    assert!(!completed_duplicate.receipts[0].accepted);
    assert_eq!(build_count.load(std::sync::atomic::Ordering::Relaxed), 3);

    tokio::time::timeout(std::time::Duration::from_millis(100), admission.shutdown())
        .await
        .expect("queued mutation generation lane must drain within 100ms")
        .expect("drain queued mutation generation lane");
}

#[tokio::test(flavor = "multi_thread")]
async fn multi_workspace_multi_session_admission_is_in_process_single_flight_and_sub_millisecond() {
    let machine_parallelism = std::thread::available_parallelism()
        .map(std::num::NonZeroUsize::get)
        .unwrap_or(2);
    let workspace_count = machine_parallelism.clamp(2, 8);
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
        move |_, _, _| {
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
                    .map_err(|_| "in-process admission release closed".to_owned())?
                    .forget();
                active_builds.fetch_sub(1, std::sync::atomic::Ordering::AcqRel);
                Ok(committed_generation())
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
                    .admit_changed_paths(
                        format!("mutation-{workspace_index}"),
                        workspace_identity,
                        root.clone(),
                        vec![root.join("src/lib.rs")],
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
    let p99 = latencies[(latencies.len() * 99 / 100).min(latencies.len() - 1)];
    eprintln!(
        "in-process-generation-admission workspaceCount={workspace_count} callsPerWorkspace={calls_per_workspace} p99Micros={}",
        p99.as_micros()
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

#[tokio::test]
async fn ready_workspace_accepts_a_new_incremental_generation_attempt() {
    let build_count = Arc::new(Mutex::new(0_u32));
    let admission = WorkspaceGenerationAdmission::new(Arc::new({
        let build_count = Arc::clone(&build_count);
        move |_workspace_identity, _project_root, _build_mode| {
            let build_count = Arc::clone(&build_count);
            Box::pin(async move {
                *build_count.lock().await += 1;
                Ok(committed_generation())
            })
        }
    }));
    let project_root = std::env::temp_dir().join("asp-generation-readmission-project");

    let first = admission
        .admit("workspace-readmission", project_root.clone())
        .await
        .expect("admit initial generation");
    assert!(first.accepted);
    assert_eq!(first.attempt, 1);
    admission
        .wait_terminal("workspace-readmission", &project_root)
        .await
        .expect("initial generation ready");

    let second = admission
        .admit("workspace-readmission", project_root.clone())
        .await
        .expect("admit incremental generation");
    assert!(second.accepted);
    assert_eq!(second.attempt, 2);
    admission
        .wait_terminal("workspace-readmission", &project_root)
        .await
        .expect("incremental generation ready");
    assert_eq!(*build_count.lock().await, 2);

    admission
        .shutdown()
        .await
        .expect("drain incremental generation admission lane");
}

#[tokio::test]
async fn ensure_observes_ready_attempt_without_starting_another_build() {
    let build_count = Arc::new(Mutex::new(0_u32));
    let admission = WorkspaceGenerationAdmission::new(Arc::new({
        let build_count = Arc::clone(&build_count);
        move |_workspace_identity, _project_root, _build_mode| {
            let build_count = Arc::clone(&build_count);
            Box::pin(async move {
                *build_count.lock().await += 1;
                Ok(committed_generation())
            })
        }
    }));
    let project_root = std::env::temp_dir().join("asp-generation-ensure-project");

    admission
        .admit("workspace-ensure", project_root.clone())
        .await
        .expect("admit generation");
    admission
        .wait_terminal("workspace-ensure", &project_root)
        .await
        .expect("wait for admitted generation");
    let ready = admission
        .ensure("workspace-ensure", &project_root)
        .await
        .expect("ensure admitted generation");

    assert_eq!(ready.state, WorkspaceGenerationAdmissionState::Ready);
    assert_eq!(ready.attempt, 1);
    assert_eq!(*build_count.lock().await, 1);
    admission.shutdown().await.expect("drain admission lane");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn ensure_schedules_once_without_waiting_for_generation_build() {
    let build_count = Arc::new(Mutex::new(0_u32));
    let release = Arc::new(Barrier::new(2));
    let admission = WorkspaceGenerationAdmission::new(Arc::new({
        let build_count = Arc::clone(&build_count);
        let release = Arc::clone(&release);
        move |_workspace_identity, _project_root, _build_mode| {
            let build_count = Arc::clone(&build_count);
            let release = Arc::clone(&release);
            Box::pin(async move {
                *build_count.lock().await += 1;
                release.wait().await;
                Ok(committed_generation())
            })
        }
    }));
    let project_root = std::env::temp_dir().join("asp-generation-ensure-submit-project");

    let started = tokio::time::Instant::now();
    let first = admission
        .ensure("workspace-ensure-submit", &project_root)
        .await
        .expect("schedule generation");
    let second = admission
        .ensure("workspace-ensure-submit", &project_root)
        .await
        .expect("observe scheduled generation");
    let submit_elapsed = started.elapsed();

    assert_eq!(first.state, WorkspaceGenerationAdmissionState::Building);
    assert_eq!(second.state, WorkspaceGenerationAdmissionState::Building);
    assert!(first.accepted);
    assert!(second.accepted);
    assert_eq!(first.attempt, 1);
    assert_eq!(second.attempt, 1);
    assert!(
        submit_elapsed < std::time::Duration::from_millis(1),
        "two generation ensure submissions must remain sub-millisecond: {submit_elapsed:?}"
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
async fn locator_publication_does_not_run_the_generation_builder() {
    use agent_semantic_client_db::runtime_server_admission_catalog::{
        RuntimeWorkspaceAdmissionCatalog, RuntimeWorkspaceAdmissionCatalogEntry,
    };
    use std::sync::atomic::{AtomicU64, Ordering};

    let fixture = tempfile::TempDir::new().expect("create locator repair fixture");
    let project_root = fixture.path().join("checkout");
    tokio::fs::create_dir_all(&project_root)
        .await
        .expect("create project root");
    let catalog_path = fixture.path().join("workspace-admissions.v1.json");
    let catalog = RuntimeWorkspaceAdmissionCatalog::load(catalog_path.clone())
        .await
        .expect("load catalog");
    let build_count = Arc::new(AtomicU64::new(0));
    let admission = WorkspaceGenerationAdmission::new(Arc::new({
        let build_count = Arc::clone(&build_count);
        move |_, _, _| {
            let build_count = Arc::clone(&build_count);
            Box::pin(async move {
                build_count.fetch_add(1, Ordering::Relaxed);
                Ok(committed_generation())
            })
        }
    }))
    .with_catalog(catalog);

    let receipt = admission
        .publish_resident_generation_locator("workspace-locator-repair", &project_root)
        .await
        .expect("publish locator without generation build");

    assert_eq!(receipt.state, WorkspaceGenerationAdmissionState::Ready);
    assert_eq!(build_count.load(Ordering::Relaxed), 0);
    assert_eq!(
        RuntimeWorkspaceAdmissionCatalog::resolve_mapped(&catalog_path, &project_root)
            .expect("resolve repaired locator"),
        RuntimeWorkspaceAdmissionCatalogEntry {
            workspace_identity: "workspace-locator-repair".to_owned(),
            project_root,
        }
    );
}

#[tokio::test]
async fn supervisor_restores_registered_workspaces_concurrently_within_budget() {
    use agent_semantic_client_db::runtime_server_admission_catalog::{
        RuntimeWorkspaceAdmissionCatalog, RuntimeWorkspaceAdmissionCatalogEntry,
    };
    use std::sync::atomic::{AtomicU64, Ordering};

    const WORKSPACE_COUNT: u64 = 16;
    let fixture = tempfile::TempDir::new().expect("create supervisor restore fixture");
    let catalog =
        RuntimeWorkspaceAdmissionCatalog::load(fixture.path().join("workspace-admissions.v1.json"))
            .await
            .expect("load supervisor catalog");
    for index in 0..WORKSPACE_COUNT {
        let project_root = fixture.path().join(format!("workspace-{index}"));
        tokio::fs::create_dir_all(&project_root)
            .await
            .expect("create workspace root");
        catalog
            .record(RuntimeWorkspaceAdmissionCatalogEntry {
                workspace_identity: format!("workspace-{index}"),
                project_root,
            })
            .await
            .expect("record workspace admission");
    }

    let active = Arc::new(AtomicU64::new(0));
    let maximum_active = Arc::new(AtomicU64::new(0));
    let build_count = Arc::new(AtomicU64::new(0));
    let admission = WorkspaceGenerationAdmission::new(Arc::new({
        let active = Arc::clone(&active);
        let maximum_active = Arc::clone(&maximum_active);
        let build_count = Arc::clone(&build_count);
        move |_, _, _| {
            let active = Arc::clone(&active);
            let maximum_active = Arc::clone(&maximum_active);
            let build_count = Arc::clone(&build_count);
            Box::pin(async move {
                let current = active.fetch_add(1, Ordering::SeqCst) + 1;
                maximum_active.fetch_max(current, Ordering::SeqCst);
                tokio::time::sleep(std::time::Duration::from_millis(2)).await;
                active.fetch_sub(1, Ordering::SeqCst);
                build_count.fetch_add(1, Ordering::SeqCst);
                Ok(committed_generation())
            })
        }
    }))
    .with_catalog(catalog);

    let started = std::time::Instant::now();
    let report = admission
        .restore_registered()
        .await
        .expect("restore registered workspace generations");
    let elapsed = started.elapsed();

    assert_eq!(report.ready.len(), WORKSPACE_COUNT as usize);
    assert!(report.failed.is_empty());
    assert_eq!(build_count.load(Ordering::SeqCst), WORKSPACE_COUNT);
    assert!(
        maximum_active.load(Ordering::SeqCst) > 1,
        "supervisor restore serialized independent workspaces"
    );
    assert!(
        elapsed < std::time::Duration::from_millis(50),
        "supervisor restore exceeded the 50ms multi-workspace gate: {elapsed:?}"
    );
    admission.shutdown().await.expect("shutdown admission");
}

#[tokio::test]
async fn failed_generation_build_retries_only_on_explicit_admission() {
    let build_count = Arc::new(Mutex::new(0_u32));
    let failed = Arc::new(tokio::sync::Notify::new());
    let admission = WorkspaceGenerationAdmission::new(Arc::new({
        let build_count = Arc::clone(&build_count);
        let failed = Arc::clone(&failed);
        move |_workspace_identity, _project_root, _build_mode| {
            let build_count = Arc::clone(&build_count);
            let failed = Arc::clone(&failed);
            Box::pin(async move {
                *build_count.lock().await += 1;
                failed.notify_one();
                Err("provider generation unavailable".to_owned())
            })
        }
    }));
    let project_root = std::env::temp_dir().join("asp-generation-admission-failure");

    admission
        .admit("workspace-sticky-failure", project_root.clone())
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
        .admit("workspace-sticky-failure", project_root)
        .await
        .expect("explicitly retry failed admission");
    assert_eq!(retry.state, WorkspaceGenerationAdmissionState::Building);
    assert!(retry.accepted);
    assert_eq!(retry.attempt, 2);
    failed.notified().await;
    assert_eq!(*build_count.lock().await, 2);

    admission
        .shutdown()
        .await
        .expect("drain failed generation admission lane");
}

#[tokio::test]
async fn shutdown_cancels_tracked_generation_builds() {
    let started = Arc::new(tokio::sync::Notify::new());
    let admission = WorkspaceGenerationAdmission::new(Arc::new({
        let started = Arc::clone(&started);
        move |_workspace_identity, _project_root, _build_mode| {
            let started = Arc::clone(&started);
            Box::pin(async move {
                started.notify_one();
                std::future::pending::<
                    Result<
                        agent_semantic_client_db::runtime_server_admission::WorkspaceGenerationCommitReceipt,
                        String,
                    >,
                >()
                .await
            })
        }
    }));
    admission
        .admit(
            "workspace-shutdown",
            std::env::temp_dir().join("asp-generation-admission-shutdown"),
        )
        .await
        .expect("schedule tracked builder");
    started.notified().await;

    assert_eq!(
        admission.shutdown().await.expect("cancel tracked builder"),
        1
    );
}
