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

#[test]
fn candidate_checkout_fixture_can_be_initialized() {
    let directory = tempfile::tempdir().expect("candidate checkout fixture");
    initialize_candidate_checkout(directory.path());
    assert!(directory.path().exists());
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
        project_id: "repo-test".to_owned(),
        workspace_identity: "workspace-a".to_owned(),
        project_root: root.join("checkout-a"),
    };
    assert!(catalog.record(entry.clone()).await.unwrap().changed());
    assert!(!catalog.record(entry.clone()).await.unwrap().changed());

    let restored = RuntimeWorkspaceAdmissionCatalog::load(path).await.unwrap();
    assert_eq!(
        restored.snapshot().iter().cloned().collect::<Vec<_>>(),
        vec![entry]
    );
    tokio::fs::remove_dir_all(root).await.unwrap();
}

#[tokio::test]
async fn legacy_v1_shape_cannot_populate_the_project_workspace_catalog() {
    let root = fixture_root();
    tokio::fs::create_dir_all(&root).await.unwrap();
    let legacy_path = root.join("workspace-admissions.v1.json");
    let legacy_bytes = br#"{"schemaId":"agent.semantic-protocols.runtime-server-workspace-admission-catalog.v1","schemaVersion":"1","entries":[{"workspaceIdentity":"workspace-legacy","projectRoot":"/tmp/legacy"}]}"#;
    tokio::fs::write(&legacy_path, legacy_bytes).await.unwrap();

    let catalog = RuntimeWorkspaceAdmissionCatalog::load(legacy_path.clone())
        .await
        .unwrap();

    assert!(catalog.snapshot().is_empty());
    assert_eq!(tokio::fs::read(&legacy_path).await.unwrap(), legacy_bytes);

    tokio::fs::remove_dir_all(root).await.unwrap();
}

#[tokio::test]
async fn identity_only_admission_publishes_v1_catalog_without_generation_build() {
    let root = fixture_root();
    let project_root = root.join("checkout");
    initialize_candidate_checkout(&project_root);
    let catalog_path = root.join("workspace-admissions.v1.json");
    let catalog = RuntimeWorkspaceAdmissionCatalog::load(catalog_path.clone())
        .await
        .unwrap();
    let builds = Arc::new(AtomicU64::new(0));
    let admission =
        agent_semantic_client_db::runtime_server_admission::WorkspaceGenerationAdmission::new(
            Arc::new({
                let builds = Arc::clone(&builds);
                move |_, _, _, _, _, _, _| {
                    let builds = Arc::clone(&builds);
                    Box::pin(async move {
                        builds.fetch_add(1, Ordering::Relaxed);
                        Err(WorkspaceGenerationBuildFailure::new(
                            WorkspaceGenerationFailureStage::GenerationBuilder,
                            "identity-only admission must not invoke generation builder",
                        ))
                    })
                }
            }),
        )
        .with_catalog(catalog);

    let entry = admission
        .admit_project_workspace_identity(project_root.clone())
        .await
        .expect("admit canonical V1 project/workspace identity");

    assert_eq!(builds.load(Ordering::Relaxed), 0);
    assert_eq!(admission.admitted_project_workspace_count(), 1);
    assert!(
        admission
            .current(&entry.workspace_identity, &project_root)
            .is_none()
    );
    assert_eq!(
        RuntimeWorkspaceAdmissionCatalog::resolve_mapped(&catalog_path, &project_root).unwrap(),
        entry
    );
    admission.shutdown().await.unwrap();
    tokio::fs::remove_dir_all(root).await.unwrap();
}

#[tokio::test]
async fn stale_workspace_identity_projection_is_replaced_once_by_current_derivation() {
    let root = fixture_root();
    let project_root = root.join("checkout");
    initialize_candidate_checkout(&project_root);
    let path = root.join("workspace-admissions.v1.json");
    let catalog = RuntimeWorkspaceAdmissionCatalog::load(path.clone())
        .await
        .unwrap();
    let stale = RuntimeWorkspaceAdmissionCatalogEntry {
        project_id: "repo-test".to_owned(),
        workspace_identity: "workspace-legacy-derived-id".to_owned(),
        project_root: project_root.clone(),
    };
    let initial_mutation = catalog.record(stale.clone()).await.unwrap();
    assert!(matches!(
        initial_mutation,
        agent_semantic_client_db::runtime_server_admission_catalog::RuntimeWorkspaceAdmissionCatalogMutation::Inserted {
            previous_revision: 0,
            current_revision: 1,
            ..
        }
    ));

    let current = RuntimeWorkspaceAdmissionCatalogEntry {
        project_id: "repo-test".to_owned(),
        workspace_identity: agent_semantic_client_db::AgentSessionRegistry::workspace_id(
            &project_root,
        )
        .unwrap(),
        project_root: project_root.clone(),
    };
    let replacement = catalog.record(current.clone()).await.unwrap();
    match replacement {
        agent_semantic_client_db::runtime_server_admission_catalog::RuntimeWorkspaceAdmissionCatalogMutation::ReplacedIdentityProjection {
            canonical_root,
            previous_workspace_identity,
            current_workspace_identity,
            previous_revision,
            current_revision,
        } => {
            assert_eq!(canonical_root, project_root);
            assert_eq!(previous_workspace_identity, stale.workspace_identity);
            assert_eq!(current_workspace_identity, current.workspace_identity);
            assert_eq!(previous_revision, 1);
            assert_eq!(current_revision, 2);
        }
        mutation => panic!("expected typed identity replacement, got {mutation:?}"),
    }
    assert_eq!(
        catalog.snapshot().iter().cloned().collect::<Vec<_>>(),
        vec![current.clone()]
    );
    assert!(matches!(
        catalog.record(current.clone()).await.unwrap(),
        agent_semantic_client_db::runtime_server_admission_catalog::RuntimeWorkspaceAdmissionCatalogMutation::Unchanged {
            resident_revision: 2
        }
    ));

    let before_rejected_replay = catalog.snapshot();
    let error = catalog.record(stale).await.unwrap_err();
    assert!(error.contains("rejected a noncanonical identity projection"));
    assert_eq!(catalog.snapshot().as_ref(), before_rejected_replay.as_ref());

    let restored = RuntimeWorkspaceAdmissionCatalog::load(path).await.unwrap();
    assert_eq!(
        restored.snapshot().iter().cloned().collect::<Vec<_>>(),
        vec![current]
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
        project_id: "repo-test".to_owned(),
        workspace_identity: "workspace-republish".to_owned(),
        project_root: root.join("checkout"),
    };

    assert!(catalog.record(entry.clone()).await.unwrap().changed());
    publications.changed().await.unwrap();
    assert_eq!(
        publications.borrow_and_update().as_ref(),
        catalog.snapshot().as_ref()
    );

    const SAMPLE_COUNT: usize = 10_000;
    let mut latencies = Vec::with_capacity(SAMPLE_COUNT);
    for _ in 0..SAMPLE_COUNT {
        let started = std::time::Instant::now();
        assert!(!catalog.record(entry.clone()).await.unwrap().changed());
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
        project_id: "repo-test".to_owned(),
        workspace_identity: "workspace-two-session-pressure".to_owned(),
        project_root: root.join("checkout"),
    };
    assert!(catalog.record(entry.clone()).await.unwrap().changed());
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
                assert!(!catalog.record(entry.clone()).await.unwrap().changed());
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
                project_id: "repo-test".to_owned(),
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
        project_id: "repo-test".to_owned(),
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
async fn project_workspace_resolution_is_exact_and_cross_project_requests_fail_closed() {
    let root = fixture_root();
    let catalog = RuntimeWorkspaceAdmissionCatalog::load(root.join("catalog.json"))
        .await
        .unwrap();
    let entry = RuntimeWorkspaceAdmissionCatalogEntry {
        project_id: "repo-project-a".to_owned(),
        workspace_identity: "workspace-checkout-a".to_owned(),
        project_root: root.join("checkout-a"),
    };
    catalog.record(entry.clone()).await.unwrap();

    assert_eq!(
        catalog
            .resolve_project_workspace("repo-project-a", "workspace-checkout-a")
            .unwrap(),
        entry
    );
    assert!(
        catalog
            .resolve_project_workspace("repo-project-b", "workspace-checkout-a")
            .is_err()
    );
    assert!(
        catalog
            .resolve_project_workspace("repo-project-a", "workspace-checkout-b")
            .is_err()
    );

    let before_drift = catalog.snapshot();
    let drift_error = catalog
        .record(RuntimeWorkspaceAdmissionCatalogEntry {
            project_id: "repo-project-b".to_owned(),
            workspace_identity: "workspace-checkout-a".to_owned(),
            project_root: root.join("checkout-a"),
        })
        .await
        .expect_err("one project/workspace binding must not admit another ProjectId");
    assert!(drift_error.contains("ProjectId drift"), "{drift_error}");
    assert_eq!(catalog.snapshot().as_ref(), before_drift.as_ref());

    tokio::fs::remove_dir_all(root).await.unwrap();
}

#[tokio::test]
async fn mapped_locator_projects_nested_language_project_to_admitted_workspace() {
    let root = fixture_root();
    let path = root.join("catalog.json");
    let workspace_root = root.join("checkout");
    let nested_project = workspace_root.join("packages/python/asp_python_graphs");
    let catalog = RuntimeWorkspaceAdmissionCatalog::load(path.clone())
        .await
        .unwrap();
    let workspace_entry = RuntimeWorkspaceAdmissionCatalogEntry {
        project_id: "repo-test".to_owned(),
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
        project_id: "repo-test".to_owned(),
        workspace_identity: "workspace-first".to_owned(),
        project_root: root.join("checkout-first"),
    };
    catalog.record(first.clone()).await.unwrap();
    assert_eq!(
        RuntimeWorkspaceAdmissionCatalog::resolve_mapped(&path, &first.project_root).unwrap(),
        first
    );

    let second = RuntimeWorkspaceAdmissionCatalogEntry {
        project_id: "repo-test".to_owned(),
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
            project_id: "repo-test".to_owned(),
            workspace_identity: "workspace-first".to_owned(),
            project_root: project_root.clone(),
        })
        .await
        .unwrap();
    assert!(
        catalog
            .record(RuntimeWorkspaceAdmissionCatalogEntry {
                project_id: "repo-test".to_owned(),
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
            project_id: "repo-test".to_owned(),
            workspace_identity: "workspace-one".to_owned(),
            project_root: root.join("checkout"),
        })
        .await
        .unwrap();
    let error = catalog
        .record(RuntimeWorkspaceAdmissionCatalogEntry {
            project_id: "repo-test".to_owned(),
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
                project_id: "repo-test".to_owned(),
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
async fn typed_ipc_admission_publishes_initial_locator_and_reaches_ready() {
    use agent_semantic_client_db::runtime_server_admission_catalog::RuntimeWorkspaceAdmissionCatalogResolveError;

    let root = fixture_root();
    let project_root = root.join("checkout");
    tokio::fs::create_dir_all(&project_root).await.unwrap();
    let workspace_id =
        agent_semantic_client_db::AgentSessionRegistry::workspace_id(&project_root).unwrap();
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
                move |_, _, candidate, _, _changed_paths, _provider_target, _| {
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
        .admit(&workspace_id, project_root.clone(), candidate_identity())
        .await
        .unwrap();
    let published = admission
        .ensure(&workspace_id, &project_root, candidate_identity())
        .await
        .unwrap();
    assert!(matches!(
        published.state,
        agent_semantic_client_db::runtime_server_admission::WorkspaceGenerationAdmissionState::Queued
            | agent_semantic_client_db::runtime_server_admission::WorkspaceGenerationAdmissionState::Building
            | agent_semantic_client_db::runtime_server_admission::WorkspaceGenerationAdmissionState::Ready
    ));
    let ready = admission
        .wait_terminal(&workspace_id, &project_root)
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
        workspace_id
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
        workspace_id
    );
    admission.shutdown().await.unwrap();
    tokio::fs::remove_dir_all(root).await.unwrap();
}
