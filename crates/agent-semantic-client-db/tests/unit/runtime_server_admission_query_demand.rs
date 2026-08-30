use agent_semantic_client_db::runtime_server_admission::{
    WorkspaceGenerationAdmission, WorkspaceGenerationAdmissionState,
    WorkspaceGenerationBuildFailure, WorkspaceGenerationCandidateIdentity,
    WorkspaceGenerationFailureStage,
};
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

fn candidate() -> WorkspaceGenerationCandidateIdentity {
    WorkspaceGenerationCandidateIdentity {
        candidate_generation: agent_semantic_runtime::git::RepositoryCandidateGeneration {
            algorithm: "blake3-worktree-state-v1".into(),
            digest: format!("blake3:{}", "7".repeat(64)),
            authorities: vec![agent_semantic_runtime::git::RepositoryCandidateAuthority::GitIndex],
        },
        policy_overlay_digest: format!("blake3:{}", "8".repeat(64)),
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn enqueue_query_demand_returns_queued_before_dispatcher_builds() {
    let started = Arc::new(tokio::sync::Notify::new());
    let builds = Arc::new(AtomicUsize::new(0));
    let admission = WorkspaceGenerationAdmission::new(Arc::new({
        let started = started.clone();
        let builds = builds.clone();
        move |_, _, _, _, _, _, _| {
            let started = started.clone();
            let builds = builds.clone();
            Box::pin(async move {
                builds.fetch_add(1, Ordering::SeqCst);
                started.notify_one();
                Err(WorkspaceGenerationBuildFailure::new(
                    WorkspaceGenerationFailureStage::GenerationBuilder,
                    "deterministic test failure",
                ))
            })
        }
    }));
    let temporary = tempfile::tempdir().expect("temporary project root");
    let root = temporary.path().to_path_buf();
    let id = "enqueue-integration".to_owned();
    let queued = admission
        .enqueue_query_demand_for_candidate(id.clone(), root.clone(), candidate(), Vec::new(), None)
        .await
        .expect("enqueue");
    assert_eq!(queued.state, WorkspaceGenerationAdmissionState::Queued);
    assert!(queued.accepted);
    assert!(admission.status(&id, &root).is_some());
    assert_eq!(builds.load(Ordering::SeqCst), 0);
    started.notified().await;
    let failed = admission.wait_terminal(&id, &root).await.expect("terminal");
    assert_eq!(failed.state, WorkspaceGenerationAdmissionState::Failed);
    assert_eq!(builds.load(Ordering::SeqCst), 1);
    admission.shutdown().await.expect("shutdown");
}
