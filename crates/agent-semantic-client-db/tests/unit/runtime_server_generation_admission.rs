use std::sync::Arc;

use agent_semantic_client_db::runtime_server_admission::{
    WorkspaceGenerationAdmission, WorkspaceGenerationAdmissionState,
};
use tokio::sync::{Barrier, Mutex};

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn concurrent_workspace_admission_is_single_flight() {
    const REQUEST_COUNT: usize = 256;
    let build_count = Arc::new(Mutex::new(0_u32));
    let release = Arc::new(Barrier::new(2));
    let admission = WorkspaceGenerationAdmission::new(Arc::new({
        let build_count = Arc::clone(&build_count);
        let release = Arc::clone(&release);
        move |_workspace_identity, _project_root| {
            let build_count = Arc::clone(&build_count);
            let release = Arc::clone(&release);
            Box::pin(async move {
                *build_count.lock().await += 1;
                release.wait().await;
                Ok(())
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
        move |_workspace_identity, project_root| {
            let roots = Arc::clone(&roots);
            Box::pin(async move {
                roots.lock().await.push(project_root);
                Ok(())
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

#[tokio::test]
async fn ready_workspace_accepts_a_new_incremental_generation_attempt() {
    let build_count = Arc::new(Mutex::new(0_u32));
    let admission = WorkspaceGenerationAdmission::new(Arc::new({
        let build_count = Arc::clone(&build_count);
        move |_workspace_identity, _project_root| {
            let build_count = Arc::clone(&build_count);
            Box::pin(async move {
                *build_count.lock().await += 1;
                Ok(())
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
        move |_workspace_identity, _project_root| {
            let build_count = Arc::clone(&build_count);
            Box::pin(async move {
                *build_count.lock().await += 1;
                Ok(())
            })
        }
    }));
    let project_root = std::env::temp_dir().join("asp-generation-ensure-project");

    admission
        .admit("workspace-ensure", project_root.clone())
        .await
        .expect("admit generation");
    let ready = admission
        .ensure("workspace-ensure", &project_root)
        .await
        .expect("ensure admitted generation");

    assert_eq!(ready.state, WorkspaceGenerationAdmissionState::Ready);
    assert_eq!(ready.attempt, 1);
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
        move |_, _| {
            let build_count = Arc::clone(&build_count);
            Box::pin(async move {
                build_count.fetch_add(1, Ordering::Relaxed);
                Ok(())
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
    let catalog = RuntimeWorkspaceAdmissionCatalog::load(
        fixture.path().join("workspace-admissions.v1.json"),
    )
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
        move |_, _| {
            let active = Arc::clone(&active);
            let maximum_active = Arc::clone(&maximum_active);
            let build_count = Arc::clone(&build_count);
            Box::pin(async move {
                let current = active.fetch_add(1, Ordering::SeqCst) + 1;
                maximum_active.fetch_max(current, Ordering::SeqCst);
                tokio::time::sleep(std::time::Duration::from_millis(2)).await;
                active.fetch_sub(1, Ordering::SeqCst);
                build_count.fetch_add(1, Ordering::SeqCst);
                Ok(())
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
        move |_workspace_identity, _project_root| {
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
        move |_workspace_identity, _project_root| {
            let started = Arc::clone(&started);
            Box::pin(async move {
                started.notify_one();
                std::future::pending::<Result<(), String>>().await
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
