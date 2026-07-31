use std::sync::Arc;

use agent_semantic_client_db::runtime_server_admission::{
    WorkspaceGenerationAdmission, WorkspaceGenerationAdmissionState,
};
use tokio::sync::{Barrier, Mutex};

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn concurrent_workspace_admission_is_single_flight() {
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
    for _ in 0..64 {
        let admission = admission.clone();
        let project_root = project_root.clone();
        requests.spawn(async move {
            admission
                .admit("workspace-single-flight", project_root)
                .await
        });
    }
    let mut accepted_count = 0_u32;
    while let Some(result) = requests.join_next().await {
        let receipt = result
            .expect("admission request task")
            .expect("admission receipt");
        assert_eq!(receipt.state, WorkspaceGenerationAdmissionState::Building);
        accepted_count += u32::from(receipt.accepted);
        receipt.validate().expect("valid admission receipt");
    }
    release.wait().await;
    assert_eq!(accepted_count, 1);
    assert_eq!(*build_count.lock().await, 1);
    let receipt = admission
        .wait_terminal("workspace-single-flight")
        .await
        .expect("wait for completed admission");
    assert_eq!(receipt.state, WorkspaceGenerationAdmissionState::Ready);
    admission
        .shutdown()
        .await
        .expect("drain generation admission lane");
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
        .wait_terminal("workspace-sticky-failure")
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
