use agent_semantic_client_db::runtime_server_admission_catalog::{
    RuntimeWorkspaceAdmissionCatalog, RuntimeWorkspaceAdmissionCatalogEntry,
};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

static FIXTURE_ID: AtomicU64 = AtomicU64::new(0);

fn fixture_root() -> std::path::PathBuf {
    std::env::temp_dir().join(format!(
        "asp-admission-catalog-{}-{}",
        std::process::id(),
        FIXTURE_ID.fetch_add(1, Ordering::Relaxed)
    ))
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
    assert_eq!(restored.entries().await, vec![entry]);
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
async fn process_cold_mapped_locator_p99_is_sub_millisecond() {
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
    let p99 = latencies[(SAMPLE_COUNT * 99 / 100).min(SAMPLE_COUNT - 1)];
    eprintln!(
        "[workspace-locator-performance] workspaces={WORKSPACE_COUNT} samples={SAMPLE_COUNT} p99Nanos={} budgetNanos=1000000",
        p99.as_nanos()
    );
    assert!(
        p99 < std::time::Duration::from_millis(1),
        "process-cold workspace locator p99 exceeded one millisecond: {p99:?}"
    );
    tokio::fs::remove_dir_all(root).await.unwrap();
}

#[tokio::test]
async fn daemon_restore_replays_each_catalog_scope_once() {
    let root = fixture_root();
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
                move |_, _, _| {
                    let builds = Arc::clone(&builds);
                    Box::pin(async move {
                        builds.fetch_add(1, Ordering::Relaxed);
                        Ok(())
                    })
                }
            }),
        )
        .with_catalog(catalog);

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
async fn typed_ipc_admission_publishes_an_initial_missing_locator() {
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
                move |_, _, _| {
                    let builds = Arc::clone(&builds);
                    Box::pin(async move {
                        builds.fetch_add(1, Ordering::Relaxed);
                        Ok(())
                    })
                }
            }),
        )
        .with_catalog(catalog);
    admission
        .admit("workspace-initial", project_root.clone())
        .await
        .unwrap();
    let ready = admission
        .ensure("workspace-initial", &project_root)
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
    admission
        .ensure("workspace-initial", &project_root)
        .await
        .unwrap();
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
            Arc::new(|workspace_identity, _, _build_mode| {
                Box::pin(async move {
                    if workspace_identity == "workspace-failed" {
                        Err("fixture canonical generation missing".to_owned())
                    } else {
                        Ok(())
                    }
                })
            }),
        )
        .with_catalog(catalog);

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
