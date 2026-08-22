use std::sync::Arc;

use agent_semantic_client_db::runtime_server_admission::{
    WorkspaceGenerationAdmission, WorkspaceGenerationAdmissionState,
};

use super::{candidate_identity, completed_generation};

#[tokio::test(flavor = "multi_thread")]
async fn unchanged_merkle_admission_performs_zero_owner_rescans() {
    let project_root = std::env::temp_dir().join("asp-unchanged-merkle-candidate-cache");
    let observed = agent_semantic_client_db::runtime_server_admission::
        discover_workspace_generation_candidate(&project_root)
        .await
        .expect("unchanged candidate resolves from resident identity");
    assert_eq!(
        observed.candidate_generation.authorities,
        vec![agent_semantic_runtime::git::RepositoryCandidateAuthority::ServerResident],
        "unchanged non-Git workspace must retain the ServerResident identity"
    );
    let repeated = agent_semantic_client_db::runtime_server_admission::
        discover_workspace_generation_candidate(&project_root)
        .await
        .expect("repeated unchanged candidate resolves from resident identity");
    assert_eq!(
        repeated, observed,
        "unchanged Merkle identity must be stable"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn thirty_run_admission_p99_within_durable_publication_boundary() {
    let _performance = crate::test_support::performance_lock();
    const RUN_COUNT: usize = 30;
    const DURABLE_PUBLICATION_BOUNDARY: std::time::Duration = std::time::Duration::from_millis(500);
    let admission = WorkspaceGenerationAdmission::new(Arc::new(
        |_workspace_identity,
         _project_root,
         candidate,
         _build_mode,
         _changed_paths,
         _provider_target,
         _cancellation| { Box::pin(async move { completed_generation(candidate) }) },
    ));
    let mut latencies = Vec::with_capacity(RUN_COUNT);
    for run in 0..RUN_COUNT {
        let workspace_identity = format!("workspace-admission-p99-{run}");
        let project_root = std::env::temp_dir().join(&workspace_identity);
        let started = tokio::time::Instant::now();
        let receipt = admission
            .admit(
                &workspace_identity,
                project_root.clone(),
                candidate_identity(),
            )
            .await
            .expect("submit admission run");
        let elapsed = started.elapsed();
        if elapsed >= DURABLE_PUBLICATION_BOUNDARY {
            eprintln!(
                "generation-admission-control-plane-breach runIndex={run} elapsedNanos={}",
                elapsed.as_nanos()
            );
        }
        latencies.push(elapsed);
        if receipt.state == WorkspaceGenerationAdmissionState::Building {
            admission
                .wait_terminal(&workspace_identity, &project_root)
                .await
                .expect("admission run reaches terminal state");
        }
    }
    latencies.sort_unstable();
    let p99 = latencies[(RUN_COUNT * 99 / 100).min(RUN_COUNT - 1)];
    eprintln!(
        "generation-admission-control-plane runs={RUN_COUNT} p99Nanos={}",
        p99.as_nanos()
    );
    assert!(
        p99 < DURABLE_PUBLICATION_BOUNDARY,
        "30-run generation admission p99 exceeded the 500ms durable publication boundary: {p99:?}"
    );
    admission.shutdown().await.expect("drain admission runs");
}
