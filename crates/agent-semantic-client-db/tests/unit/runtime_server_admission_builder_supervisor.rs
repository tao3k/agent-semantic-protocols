// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use std::sync::Arc;

use agent_semantic_client_db::runtime_generation_cancellation::GenerationCancellation;
use agent_semantic_client_db::runtime_server_admission::WorkspaceGenerationBuildFailure;
use agent_semantic_client_db::runtime_server_admission::WorkspaceGenerationBuildFuture;
use agent_semantic_client_db::runtime_server_admission::WorkspaceGenerationBuildMode;
use agent_semantic_client_db::runtime_server_admission::WorkspaceGenerationCandidateIdentity;
use agent_semantic_client_db::runtime_server_admission::WorkspaceGenerationFailureStage;

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
async fn runtime_cancellation_terminates_the_generation_builder() {
    let cancellation = GenerationCancellation::new();
    let cancelled = cancellation.clone();
    let builder = Arc::new(
        move |_,
              _,
              _,
              _,
              _changed_paths,
              _provider_target,
              _cancellation|
              -> WorkspaceGenerationBuildFuture {
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
    let task = tokio::spawn(super::run(
        builder,
        "workspace".to_owned(),
        std::path::PathBuf::from("/workspace"),
        candidate(),
        WorkspaceGenerationBuildMode::RestoreOrBuild,
        std::sync::Arc::new(std::collections::BTreeSet::new()),
        None,
        cancellation.clone(),
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
