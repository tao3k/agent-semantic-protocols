use std::sync::Arc;

use agent_semantic_client_db::runtime_server_admission::{
    WorkspaceGenerationAdmission, WorkspaceGenerationAdmissionState,
};

use super::{candidate_identity, completed_generation};

#[tokio::test(flavor = "multi_thread")]
async fn concurrent_generation_rebuild_admission_is_single_flight_and_sub_millisecond() {
    const REQUEST_COUNT: usize = 4_096;
    let build_count = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let release_repair = Arc::new(tokio::sync::Notify::new());
    let admission = Arc::new(WorkspaceGenerationAdmission::new(Arc::new({
        let build_count = Arc::clone(&build_count);
        let release_repair = Arc::clone(&release_repair);
        move |_, _, candidate, _, _changed_paths, _cancellation| {
            let build_count = Arc::clone(&build_count);
            let release_repair = Arc::clone(&release_repair);
            Box::pin(async move {
                let attempt = build_count.fetch_add(1, std::sync::atomic::Ordering::AcqRel) + 1;
                if attempt == 2 {
                    release_repair.notified().await;
                }
                completed_generation(candidate)
            })
        }
    })));
    let project_root = std::env::temp_dir().join("asp-ready-locator-reconciliation");

    admission
        .admit(
            "workspace-ready-locator-reconciliation",
            project_root.clone(),
            candidate_identity(),
        )
        .await
        .expect("admit initial generation");
    admission
        .wait_terminal("workspace-ready-locator-reconciliation", &project_root)
        .await
        .expect("initial generation reaches Ready");

    let mut requests = tokio::task::JoinSet::new();
    for _ in 0..REQUEST_COUNT {
        let admission = Arc::clone(&admission);
        let project_root = project_root.clone();
        requests.spawn(async move {
            let started = tokio::time::Instant::now();
            let receipt = admission
                .admit_cache_rebuild(
                    "ready-locator-reconciliation",
                    "workspace-ready-locator-reconciliation",
                    project_root,
                    candidate_identity(),
                )
                .await;
            (receipt, started.elapsed())
        });
    }
    let mut accepted = 0_usize;
    let mut latencies = Vec::with_capacity(REQUEST_COUNT);
    while let Some(result) = requests.join_next().await {
        let (receipt, elapsed) = result.expect("join locator reconciliation request");
        let receipt = receipt.expect("admit locator reconciliation");
        assert_eq!(receipt.state, WorkspaceGenerationAdmissionState::Building);
        assert_eq!(receipt.attempt, 2);
        accepted += usize::from(receipt.accepted);
        latencies.push(elapsed);
    }
    latencies.sort_unstable();
    let p99 = latencies[(latencies.len() * 99 / 100).min(latencies.len() - 1)];
    assert_eq!(accepted, 1);
    assert_eq!(build_count.load(std::sync::atomic::Ordering::Acquire), 2);
    assert!(
        p99 < std::time::Duration::from_millis(1),
        "ready locator reconciliation p99 must remain sub-millisecond: {p99:?}"
    );

    release_repair.notify_one();
    let ready = admission
        .wait_terminal("workspace-ready-locator-reconciliation", &project_root)
        .await
        .expect("reconciled generation reaches Ready");
    assert_eq!(ready.state, WorkspaceGenerationAdmissionState::Ready);
    assert_eq!(ready.attempt, 2);
    admission.shutdown().await.expect("drain admission lane");
}
