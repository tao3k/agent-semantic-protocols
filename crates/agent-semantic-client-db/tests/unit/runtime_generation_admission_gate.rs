use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use agent_semantic_client_db::runtime_server_admission::{
    WorkspaceGenerationAdmission, WorkspaceGenerationAdmissionState,
};

#[tokio::test(flavor = "multi_thread")]
async fn multi_workspace_multi_session_admission_is_single_flight_and_sub_millisecond() {
    let machine_parallelism = std::thread::available_parallelism()
        .map(std::num::NonZeroUsize::get)
        .unwrap_or(2);
    let workspace_count = machine_parallelism.clamp(2, 8);
    let calls_per_workspace = machine_parallelism.saturating_mul(16).max(32);
    let active_builds = Arc::new(AtomicUsize::new(0));
    let peak_builds = Arc::new(AtomicUsize::new(0));
    let build_count = Arc::new(AtomicUsize::new(0));
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
                build_count.fetch_add(1, Ordering::Relaxed);
                let active = active_builds.fetch_add(1, Ordering::AcqRel) + 1;
                peak_builds.fetch_max(active, Ordering::AcqRel);
                release
                    .acquire_owned()
                    .await
                    .map_err(|_| "admission fixture release closed".to_owned())?
                    .forget();
                active_builds.fetch_sub(1, Ordering::AcqRel);
                Ok(
                    agent_semantic_client_db::runtime_server_admission::WorkspaceGenerationCommitReceipt {
                        active_epoch: 1,
                        generation_digest: "blake3-256:1111111111111111111111111111111111111111111111111111111111111111".to_owned(),
                        source_root_digest: "blake3-256:2222222222222222222222222222222222222222222222222222222222222222".to_owned(),
                    },
                )
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
    tokio::time::timeout(Duration::from_millis(100), async {
        while let Some(joined) = requests.join_next().await {
            let (workspace_index, receipt, latency) = joined.expect("join admission request");
            let receipt = receipt.expect("admit workspace mutation");
            accepted_per_workspace[workspace_index] += usize::from(
                receipt
                    .receipts
                    .first()
                    .is_some_and(|receipt| receipt.accepted),
            );
            latencies.push(latency);
        }
    })
    .await
    .expect("all in-process admission submissions must complete within 100ms");
    assert!(accepted_per_workspace.iter().all(|accepted| *accepted == 1));
    latencies.sort_unstable();
    let p99 = latencies[(latencies.len() * 99 / 100).min(latencies.len() - 1)];
    eprintln!(
        "resident-admission-gate workspaceCount={workspace_count} callsPerWorkspace={calls_per_workspace} p99Micros={}",
        p99.as_micros()
    );
    assert!(
        p99 < Duration::from_millis(1),
        "resident admission p99 must remain sub-millisecond: {p99:?}"
    );

    tokio::time::timeout(Duration::from_millis(100), async {
        while active_builds.load(Ordering::Acquire) < workspace_count {
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("different workspaces must enter independent writer lanes within 100ms");
    assert_eq!(peak_builds.load(Ordering::Acquire), workspace_count);
    assert_eq!(build_count.load(Ordering::Acquire), workspace_count);

    release.add_permits(workspace_count);
    for workspace_index in 0..workspace_count {
        let workspace_identity = format!("workspace-{workspace_index}");
        let root = std::env::temp_dir().join(&workspace_identity);
        let terminal = tokio::time::timeout(
            Duration::from_millis(100),
            admission.wait_terminal(&workspace_identity, &root),
        )
        .await
        .expect("workspace admission must become terminal within 100ms")
        .expect("workspace admission terminal receipt");
        assert_eq!(terminal.state, WorkspaceGenerationAdmissionState::Ready);
    }
    let workspace_identity = "workspace-0";
    let root = std::env::temp_dir().join(workspace_identity);
    let identity_reuse_error = admission
        .admit_changed_paths(
            "mutation-0",
            workspace_identity,
            root.clone(),
            vec![root.join("src/different.rs")],
        )
        .await
        .expect_err("one mutation identity cannot name two changed-path sets");
    assert!(identity_reuse_error.contains("different changed-path set"));
    tokio::time::timeout(Duration::from_millis(100), admission.shutdown())
        .await
        .expect("resident admission lanes must drain within 100ms")
        .expect("drain resident admission lanes");
}
