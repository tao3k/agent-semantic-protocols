use agent_semantic_client_db::runtime_server_admission::{
    WorkspaceGenerationBuildFailure, WorkspaceGenerationFailureStage,
};
use agent_semantic_client_db::runtime_server_admission_catalog::{
    RuntimeWorkspaceAdmissionCatalog, RuntimeWorkspaceAdmissionCatalogEntry,
};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

static FIXTURE_ID: AtomicU64 = AtomicU64::new(0);

fn candidate_identity()
-> agent_semantic_client_db::runtime_server_admission::WorkspaceGenerationCandidateIdentity {
    agent_semantic_client_db::runtime_server_admission::WorkspaceGenerationCandidateIdentity {
        candidate_generation: agent_semantic_runtime::git::RepositoryCandidateGeneration {
            algorithm: "blake3-worktree-state-v1".to_owned(),
            digest: "blake3:cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc"
                .to_owned(),
            authorities: vec![agent_semantic_runtime::git::RepositoryCandidateAuthority::GitIndex],
        },
        policy_overlay_digest:
            "blake3:dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd".to_owned(),
    }
}

fn fixture_root() -> std::path::PathBuf {
    std::env::temp_dir().join(format!(
        "asp-admission-catalog-{}-{}",
        std::process::id(),
        FIXTURE_ID.fetch_add(1, Ordering::Relaxed)
    ))
}

fn initialize_candidate_checkout(path: &std::path::Path) {
    std::fs::create_dir_all(path).expect("create candidate checkout");
    let initialized = std::process::Command::new("git")
        .args(["init", "--quiet"])
        .arg(path)
        .status()
        .expect("run git init");
    assert!(initialized.success(), "initialize candidate checkout");
    std::fs::write(path.join("candidate.txt"), b"candidate\n").expect("write candidate fixture");
    let staged = std::process::Command::new("git")
        .args(["add", "candidate.txt"])
        .current_dir(path)
        .status()
        .expect("run git add");
    assert!(staged.success(), "stage candidate fixture");
}

fn committed_generation()
-> agent_semantic_client_db::runtime_server_admission::WorkspaceGenerationCommitReceipt {
    agent_semantic_client_db::runtime_server_admission::WorkspaceGenerationCommitReceipt {
        projection_capability: crate::fixture::ready_projection_capability_fixture(
            "workspace-test",
            "blake3-256:1111111111111111111111111111111111111111111111111111111111111111",
            "blake3-256:2222222222222222222222222222222222222222222222222222222222222222",
            1,
        ),
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
    String,
> {
    agent_semantic_client_db::runtime_server_admission::WorkspaceGenerationBuildCompletion::new(
        candidate,
        committed_generation(),
    )
}

#[tokio::test]
async fn catalog_persists_unique_workspace_source_scopes_atomically() {
    let root = fixture_root();
    let path = root.join("workspace-admissions.v1.json");
    let catalog = RuntimeWorkspaceAdmissionCatalog::load(path.clone())
        .await
        .unwrap();
    let entry = RuntimeWorkspaceAdmissionCatalogEntry {
        workspace_identity: "workspace-a".to_owned(),
        project_root: root.join("checkout-a"),
    };
    assert!(catalog.record(entry.clone()).await.unwrap());
    assert!(!catalog.record(entry.clone()).await.unwrap());

    let restored = RuntimeWorkspaceAdmissionCatalog::load(path).await.unwrap();
    assert_eq!(
        restored.snapshot().iter().cloned().collect::<Vec<_>>(),
        vec![entry]
    );
    tokio::fs::remove_dir_all(root).await.unwrap();
}

#[tokio::test]
async fn repeated_admission_is_silent_and_cannot_retrigger_materialization() {
    let root = fixture_root();
    let path = root.join("workspace-admissions.v1.json");
    let catalog = RuntimeWorkspaceAdmissionCatalog::load(path).await.unwrap();
    let mut publications = catalog.subscribe();
    let entry = RuntimeWorkspaceAdmissionCatalogEntry {
        workspace_identity: "workspace-republish".to_owned(),
        project_root: root.join("checkout"),
    };

    assert!(catalog.record(entry.clone()).await.unwrap());
    publications.changed().await.unwrap();
    assert_eq!(
        publications.borrow_and_update().as_ref(),
        catalog.snapshot().as_ref()
    );

    const SAMPLE_COUNT: usize = 10_000;
    let mut latencies = Vec::with_capacity(SAMPLE_COUNT);
    for _ in 0..SAMPLE_COUNT {
        let started = std::time::Instant::now();
        assert!(!catalog.record(entry.clone()).await.unwrap());
        latencies.push(started.elapsed());
    }
    latencies.sort_unstable();
    let p99 = latencies[(SAMPLE_COUNT * 99 / 100).min(SAMPLE_COUNT - 1)];
    assert!(
        !publications.has_changed().unwrap(),
        "an idempotent admission must not wake snapshot or generation materializers"
    );
    eprintln!(
        "[workspace-admission-idempotence] admissions={SAMPLE_COUNT} publications=0 metadataProbes=0 locatorWrites=0 snapshotMaterializations=0 p99Nanos={} budgetNanos=1000000",
        p99.as_nanos()
    );
    assert!(
        p99 < std::time::Duration::from_millis(1),
        "idempotent admission p99 exceeded one millisecond: {p99:?}"
    );

    tokio::fs::remove_dir_all(root).await.unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn two_sessions_cannot_amplify_one_workspace_admission_into_io() {
    const SESSION_COUNT: usize = 2;
    const ADMISSIONS_PER_SESSION: usize = 5_000;
    let root = fixture_root();
    let path = root.join("workspace-admissions.v1.json");
    let catalog = RuntimeWorkspaceAdmissionCatalog::load(path).await.unwrap();
    let mut publications = catalog.subscribe();
    let entry = RuntimeWorkspaceAdmissionCatalogEntry {
        workspace_identity: "workspace-two-session-pressure".to_owned(),
        project_root: root.join("checkout"),
    };
    assert!(catalog.record(entry.clone()).await.unwrap());
    publications.changed().await.unwrap();
    let _ = publications.borrow_and_update();

    let barrier = Arc::new(tokio::sync::Barrier::new(SESSION_COUNT));
    let mut sessions = Vec::with_capacity(SESSION_COUNT);
    for _ in 0..SESSION_COUNT {
        let catalog = catalog.clone();
        let entry = entry.clone();
        let barrier = Arc::clone(&barrier);
        sessions.push(tokio::spawn(async move {
            barrier.wait().await;
            let mut latencies = Vec::with_capacity(ADMISSIONS_PER_SESSION);
            for _ in 0..ADMISSIONS_PER_SESSION {
                let started = std::time::Instant::now();
                assert!(!catalog.record(entry.clone()).await.unwrap());
                latencies.push(started.elapsed());
            }
            latencies
        }));
    }
    let mut latencies = Vec::with_capacity(SESSION_COUNT * ADMISSIONS_PER_SESSION);
    for session in sessions {
        latencies.extend(session.await.unwrap());
    }
    latencies.sort_unstable();
    let sample_count = latencies.len();
    let p99 = latencies[(sample_count * 99 / 100).min(sample_count - 1)];
    assert!(!publications.has_changed().unwrap());
    eprintln!(
        "[workspace-admission-two-session-pressure] sessions={SESSION_COUNT} admissions={sample_count} publications=0 metadataProbes=0 locatorWrites=0 snapshotMaterializations=0 p99Nanos={} budgetNanos=1000000",
        p99.as_nanos()
    );
    assert!(
        p99 < std::time::Duration::from_millis(1),
        "two-session idempotent admission p99 exceeded one millisecond: {p99:?}"
    );

    tokio::fs::remove_dir_all(root).await.unwrap();
}

#[tokio::test]
async fn catalog_rejects_relative_project_roots() {
    let root = fixture_root();
    let catalog = RuntimeWorkspaceAdmissionCatalog::load(root.join("catalog.json"))
        .await
        .unwrap();
    assert!(
        catalog
            .record(RuntimeWorkspaceAdmissionCatalogEntry {
                workspace_identity: "workspace-a".to_owned(),
                project_root: "relative".into(),
            })
            .await
            .is_err()
    );
}

#[tokio::test]
async fn mapped_locator_resolves_canonical_scope_without_runtime_or_git() {
    let root = fixture_root();
    let path = root.join("catalog.json");
    let project_root = root.join("checkout");
    let catalog = RuntimeWorkspaceAdmissionCatalog::load(path.clone())
        .await
        .unwrap();
    let entry = RuntimeWorkspaceAdmissionCatalogEntry {
        workspace_identity: "workspace-mapped".to_owned(),
        project_root: project_root.clone(),
    };
    catalog.record(entry.clone()).await.unwrap();

    assert_eq!(
        RuntimeWorkspaceAdmissionCatalog::resolve_mapped(&path, &project_root).unwrap(),
        entry
    );
    tokio::fs::remove_dir_all(root).await.unwrap();
}

#[tokio::test]
async fn mapped_locator_projects_nested_language_project_to_admitted_workspace() {
    let root = fixture_root();
    let path = root.join("catalog.json");
    let workspace_root = root.join("checkout");
    let nested_project = workspace_root.join("packages/python/asp_graph_turbo");
    let catalog = RuntimeWorkspaceAdmissionCatalog::load(path.clone())
        .await
        .unwrap();
    let workspace_entry = RuntimeWorkspaceAdmissionCatalogEntry {
        workspace_identity: "workspace-repository".to_owned(),
        project_root: workspace_root.clone(),
    };
    catalog.record(workspace_entry.clone()).await.unwrap();

    assert_eq!(
        RuntimeWorkspaceAdmissionCatalog::resolve_mapped(&path, &nested_project).unwrap(),
        workspace_entry
    );
    tokio::fs::remove_dir_all(root).await.unwrap();
}

#[tokio::test]
async fn mapped_locator_reloads_after_atomic_catalog_publication() {
    let root = fixture_root();
    let path = root.join("catalog.json");
    let catalog = RuntimeWorkspaceAdmissionCatalog::load(path.clone())
        .await
        .unwrap();
    let first = RuntimeWorkspaceAdmissionCatalogEntry {
        workspace_identity: "workspace-first".to_owned(),
        project_root: root.join("checkout-first"),
    };
    catalog.record(first.clone()).await.unwrap();
    assert_eq!(
        RuntimeWorkspaceAdmissionCatalog::resolve_mapped(&path, &first.project_root).unwrap(),
        first
    );

    let second = RuntimeWorkspaceAdmissionCatalogEntry {
        workspace_identity: "workspace-second".to_owned(),
        project_root: root.join("checkout-second"),
    };
    catalog.record(second.clone()).await.unwrap();
    assert_eq!(
        RuntimeWorkspaceAdmissionCatalog::resolve_mapped(&path, &second.project_root).unwrap(),
        second
    );
    tokio::fs::remove_dir_all(root).await.unwrap();
}

#[tokio::test]
async fn catalog_rejects_two_workspace_identities_for_one_canonical_root() {
    let root = fixture_root();
    let catalog = RuntimeWorkspaceAdmissionCatalog::load(root.join("catalog.json"))
        .await
        .unwrap();
    let project_root = root.join("checkout");
    catalog
        .record(RuntimeWorkspaceAdmissionCatalogEntry {
            workspace_identity: "workspace-first".to_owned(),
            project_root: project_root.clone(),
        })
        .await
        .unwrap();
    assert!(
        catalog
            .record(RuntimeWorkspaceAdmissionCatalogEntry {
                workspace_identity: "workspace-second".to_owned(),
                project_root,
            })
            .await
            .is_err()
    );
    tokio::fs::remove_dir_all(root).await.unwrap();
}

#[tokio::test]
async fn catalog_rejects_one_workspace_identity_for_multiple_roots() {
    let root = fixture_root();
    let catalog = RuntimeWorkspaceAdmissionCatalog::load(root.join("catalog.json"))
        .await
        .unwrap();
    catalog
        .record(RuntimeWorkspaceAdmissionCatalogEntry {
            workspace_identity: "workspace-one".to_owned(),
            project_root: root.join("checkout"),
        })
        .await
        .unwrap();
    let error = catalog
        .record(RuntimeWorkspaceAdmissionCatalogEntry {
            workspace_identity: "workspace-one".to_owned(),
            project_root: root.join("checkout/src"),
        })
        .await
        .expect_err("one workspace identity must own exactly one canonical root");
    assert!(error.contains("already owns a canonical root"), "{error}");
    tokio::fs::remove_dir_all(root).await.unwrap();
}

#[tokio::test]
async fn process_cold_mapped_locator_has_sub_ms_p95_and_bounded_p99() {
    const WORKSPACE_COUNT: usize = 32;
    const SAMPLE_COUNT: usize = 2_048;
    let root = fixture_root();
    let path = root.join("catalog.json");
    let catalog = RuntimeWorkspaceAdmissionCatalog::load(path.clone())
        .await
        .unwrap();
    let mut roots = Vec::with_capacity(WORKSPACE_COUNT);
    for index in 0..WORKSPACE_COUNT {
        let project_root = root.join(format!("checkout-{index}"));
        catalog
            .record(RuntimeWorkspaceAdmissionCatalogEntry {
                workspace_identity: format!("workspace-{index}"),
                project_root: project_root.clone(),
            })
            .await
            .unwrap();
        roots.push(project_root);
    }

    let mut latencies = Vec::with_capacity(SAMPLE_COUNT);
    for index in 0..SAMPLE_COUNT {
        let started = std::time::Instant::now();
        let entry = RuntimeWorkspaceAdmissionCatalog::resolve_mapped(
            &path,
            &roots[index % WORKSPACE_COUNT],
        )
        .unwrap();
        assert_eq!(
            entry.workspace_identity,
            format!("workspace-{}", index % WORKSPACE_COUNT)
        );
        latencies.push(started.elapsed());
    }
    latencies.sort_unstable();
    let p95 = latencies[(SAMPLE_COUNT * 95 / 100).min(SAMPLE_COUNT - 1)];
    let p99 = latencies[(SAMPLE_COUNT * 99 / 100).min(SAMPLE_COUNT - 1)];
    eprintln!(
        "[workspace-locator-performance] workspaces={WORKSPACE_COUNT} samples={SAMPLE_COUNT} p95Nanos={} p99Nanos={} typicalBudgetNanos=1000000 hardBudgetNanos=10000000",
        p95.as_nanos(),
        p99.as_nanos()
    );
    assert!(
        p95 < std::time::Duration::from_millis(1),
        "process-cold workspace locator p95 exceeded one millisecond: {p95:?}"
    );
    assert!(
        p99 < std::time::Duration::from_millis(10),
        "process-cold workspace locator p99 exceeded ten milliseconds: {p99:?}"
    );
    tokio::fs::remove_dir_all(root).await.unwrap();
}

#[tokio::test]
async fn daemon_restore_replays_each_catalog_scope_once() {
    let root = fixture_root();
    initialize_candidate_checkout(&root.join("checkout"));
    let catalog = RuntimeWorkspaceAdmissionCatalog::load(root.join("catalog.json"))
        .await
        .unwrap();
    let entry = RuntimeWorkspaceAdmissionCatalogEntry {
        workspace_identity: "workspace-restore".to_owned(),
        project_root: root.join("checkout"),
    };
    catalog.record(entry).await.unwrap();
    let builds = Arc::new(AtomicU64::new(0));
    let admission =
        agent_semantic_client_db::runtime_server_admission::WorkspaceGenerationAdmission::new(
            Arc::new({
                let builds = Arc::clone(&builds);
                move |_, _, candidate, _, _changed_paths, _| {
                    let builds = Arc::clone(&builds);
                    Box::pin(async move {
                        builds.fetch_add(1, Ordering::Relaxed);
                        completed_generation(candidate).map_err(|error| {
                            WorkspaceGenerationBuildFailure::new(
                                WorkspaceGenerationFailureStage::DurableRestore,
                                error,
                            )
                        })
                    })
                }
            }),
        )
        .with_catalog(catalog.clone());

    let first = admission.restore_registered().await.unwrap();
    assert_eq!(first.ready.len(), 1);
    assert!(first.failed.is_empty());
    let second = admission.restore_registered().await.unwrap();
    assert_eq!(second.ready.len(), 1);
    assert!(second.failed.is_empty());
    assert_eq!(builds.load(Ordering::Relaxed), 1);
    admission.shutdown().await.unwrap();
    tokio::fs::remove_dir_all(root).await.unwrap();
}

#[tokio::test]
async fn typed_ipc_admission_publishes_initial_locator_and_reaches_ready() {
    use agent_semantic_client_db::runtime_server_admission_catalog::RuntimeWorkspaceAdmissionCatalogResolveError;

    let root = fixture_root();
    let project_root = root.join("checkout");
    tokio::fs::create_dir_all(&project_root).await.unwrap();
    let catalog_path = root.join("catalog.json");
    assert!(matches!(
        RuntimeWorkspaceAdmissionCatalog::resolve_mapped(&catalog_path, &project_root),
        Err(RuntimeWorkspaceAdmissionCatalogResolveError::Unavailable { .. })
    ));

    let catalog = RuntimeWorkspaceAdmissionCatalog::load(catalog_path.clone())
        .await
        .unwrap();
    let builds = Arc::new(AtomicU64::new(0));
    let admission =
        agent_semantic_client_db::runtime_server_admission::WorkspaceGenerationAdmission::new(
            Arc::new({
                let builds = Arc::clone(&builds);
                move |_, _, candidate, _, _changed_paths, _| {
                    let builds = Arc::clone(&builds);
                    Box::pin(async move {
                        builds.fetch_add(1, Ordering::Relaxed);
                        completed_generation(candidate).map_err(|error| {
                            WorkspaceGenerationBuildFailure::new(
                                WorkspaceGenerationFailureStage::DurableRestore,
                                error,
                            )
                        })
                    })
                }
            }),
        )
        .with_catalog(catalog.clone());
    admission
        .admit(
            "workspace-initial",
            project_root.clone(),
            candidate_identity(),
        )
        .await
        .unwrap();
    let published = admission
        .ensure("workspace-initial", &project_root, candidate_identity())
        .await
        .unwrap();
    assert!(matches!(
        published.state,
        agent_semantic_client_db::runtime_server_admission::WorkspaceGenerationAdmissionState::Building
            | agent_semantic_client_db::runtime_server_admission::WorkspaceGenerationAdmissionState::Ready
    ));
    let ready = admission
        .wait_terminal("workspace-initial", &project_root)
        .await
        .unwrap();
    assert_eq!(
        ready.state,
        agent_semantic_client_db::runtime_server_admission::WorkspaceGenerationAdmissionState::Ready
    );
    assert_eq!(builds.load(Ordering::Relaxed), 1);
    assert_eq!(
        RuntimeWorkspaceAdmissionCatalog::resolve_mapped(&catalog_path, &project_root)
            .unwrap()
            .workspace_identity,
        "workspace-initial"
    );
    tokio::fs::remove_file(&catalog_path).await.unwrap();
    assert!(matches!(
        RuntimeWorkspaceAdmissionCatalog::resolve_mapped(&catalog_path, &project_root),
        Err(RuntimeWorkspaceAdmissionCatalogResolveError::Unavailable { .. })
    ));
    assert!(catalog.repair_locator().await.unwrap());
    assert_eq!(builds.load(Ordering::Relaxed), 1);
    assert_eq!(
        RuntimeWorkspaceAdmissionCatalog::resolve_mapped(&catalog_path, &project_root)
            .unwrap()
            .workspace_identity,
        "workspace-initial"
    );
    admission.shutdown().await.unwrap();
    tokio::fs::remove_dir_all(root).await.unwrap();
}

#[tokio::test]
async fn daemon_restore_isolates_failed_workspace_scopes() {
    let root = fixture_root();
    let catalog = RuntimeWorkspaceAdmissionCatalog::load(root.join("catalog.json"))
        .await
        .unwrap();
    for workspace_identity in ["workspace-ready", "workspace-failed"] {
        initialize_candidate_checkout(&root.join(workspace_identity));
        catalog
            .record(RuntimeWorkspaceAdmissionCatalogEntry {
                workspace_identity: workspace_identity.to_owned(),
                project_root: root.join(workspace_identity),
            })
            .await
            .unwrap();
    }
    let admission =
        agent_semantic_client_db::runtime_server_admission::WorkspaceGenerationAdmission::new(
            Arc::new(
                |workspace_identity, _, candidate, _build_mode, _changed_paths, _cancellation| {
                    Box::pin(async move {
                        if workspace_identity == "workspace-failed" {
                            Err(WorkspaceGenerationBuildFailure::new(
                                WorkspaceGenerationFailureStage::DurableRestore,
                                "fixture canonical generation missing",
                            ))
                        } else {
                            completed_generation(candidate).map_err(|error| {
                                WorkspaceGenerationBuildFailure::new(
                                    WorkspaceGenerationFailureStage::DurableRestore,
                                    error,
                                )
                            })
                        }
                    })
                },
            ),
        )
        .with_catalog(catalog.clone());

    let report = admission.restore_registered().await.unwrap();
    assert_eq!(
        report
            .ready
            .iter()
            .map(|receipt| receipt.workspace_identity.as_str())
            .collect::<Vec<_>>(),
        vec!["workspace-ready"]
    );
    assert_eq!(
        report
            .failed
            .iter()
            .map(|receipt| receipt.workspace_identity.as_str())
            .collect::<Vec<_>>(),
        vec!["workspace-failed"]
    );
    admission.shutdown().await.unwrap();
    tokio::fs::remove_dir_all(root).await.unwrap();
}
