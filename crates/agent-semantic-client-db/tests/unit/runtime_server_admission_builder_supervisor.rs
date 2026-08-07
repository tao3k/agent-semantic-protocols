use std::sync::Arc;
use std::time::Duration;

use agent_semantic_client_db::runtime_generation_cancellation::GenerationCancellation;
use agent_semantic_client_db::runtime_server_admission::{
    WorkspaceGenerationBuildFailure, WorkspaceGenerationBuildFuture, WorkspaceGenerationBuildMode,
    WorkspaceGenerationCandidateIdentity, WorkspaceGenerationFailureStage,
};

fn candidate() -> WorkspaceGenerationCandidateIdentity {
    WorkspaceGenerationCandidateIdentity {
        candidate_generation: agent_semantic_runtime::git::RepositoryCandidateGeneration {
            algorithm: "blake3-worktree-state-v1".to_owned(),
            digest: format!("blake3:{}", "0".repeat(64)),
            authorities: vec![agent_semantic_runtime::git::RepositoryCandidateAuthority::GitIndex],
        },
        policy_overlay_digest: format!("blake3:{}", "1".repeat(64)),
    }
}

#[tokio::test]
async fn delayed_builder_hits_short_deadline_with_typed_reason() {
    let builder = Arc::new(
        |_, _, _, _, _changed_paths, _cancellation| -> WorkspaceGenerationBuildFuture {
            Box::pin(async {
                std::future::pending::<Result<_, WorkspaceGenerationBuildFailure>>().await
            })
        },
    );
    let result = super::run_with_deadline(
        builder,
        "workspace".to_owned(),
        std::path::PathBuf::from("/workspace"),
        candidate(),
        WorkspaceGenerationBuildMode::RestoreOrBuild,
        GenerationCancellation::new(),
        Duration::from_millis(5),
    )
    .await;
    let error = match result {
        Ok(_) => panic!("deadline should fail"),
        Err(error) => error,
    };
    assert_eq!(
        error.stage,
        WorkspaceGenerationFailureStage::GenerationBuilderSupervision
    );
}

#[tokio::test]
async fn cancellation_remains_distinct_from_deadline() {
    let cancellation = GenerationCancellation::new();
    let cancelled = cancellation.clone();
    let builder = Arc::new(
        move |_, _, _, _, _changed_paths, _cancellation| -> WorkspaceGenerationBuildFuture {
            let cancelled = cancelled.clone();
            Box::pin(async move {
                cancelled.cancelled().await;
                Err(WorkspaceGenerationBuildFailure::new(
                    WorkspaceGenerationFailureStage::GenerationBuilderSupervision,
                    "cancelled",
                ))
            })
        },
    );
    let task = tokio::spawn(super::run_with_deadline(
        builder,
        "workspace".to_owned(),
        std::path::PathBuf::from("/workspace"),
        candidate(),
        WorkspaceGenerationBuildMode::RestoreOrBuild,
        cancellation.clone(),
        Duration::from_secs(1),
    ));
    cancellation.cancel();
    let error = match task.await.expect("join") {
        Ok(_) => panic!("cancelled"),
        Err(error) => error,
    };
    assert_eq!(
        error.stage,
        WorkspaceGenerationFailureStage::GenerationBuilderSupervision
    );
    assert_eq!(error.message, "workspace generation build cancelled");
}
