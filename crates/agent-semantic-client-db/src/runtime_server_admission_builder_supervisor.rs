// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Tokio supervision boundary for one workspace generation builder.

use std::path::PathBuf;

use super::runtime_server_admission::{
    WorkspaceGenerationBuildCompletion, WorkspaceGenerationBuildFailure,
    WorkspaceGenerationBuildMode, WorkspaceGenerationBuilder, WorkspaceGenerationCandidateIdentity,
    WorkspaceGenerationFailureStage,
};

#[expect(
    clippy::too_many_arguments,
    reason = "the supervisor boundary keeps generation identity and cancellation authorities explicit"
)]
pub(super) async fn run(
    builder: WorkspaceGenerationBuilder,
    workspace_identity: String,
    project_root: PathBuf,
    candidate: WorkspaceGenerationCandidateIdentity,
    build_mode: WorkspaceGenerationBuildMode,
    changed_paths: std::sync::Arc<std::collections::BTreeSet<PathBuf>>,
    provider_target: Option<crate::runtime_server_admission::WorkspaceGenerationProviderTarget>,
    cancellation: crate::runtime_generation_cancellation::GenerationCancellation,
) -> Result<WorkspaceGenerationBuildCompletion, WorkspaceGenerationBuildFailure> {
    let mut builders = tokio::task::JoinSet::new();
    builders.spawn(builder(
        workspace_identity,
        project_root,
        candidate,
        build_mode,
        changed_paths,
        provider_target,
        cancellation.clone(),
    ));
    let outcome = tokio::select! {
        _ = cancellation.cancelled() => None,
        result = builders.join_next() => result,
    };
    match outcome {
        None if cancellation.is_cancelled() => Err(WorkspaceGenerationBuildFailure::new(
            WorkspaceGenerationFailureStage::GenerationBuilderSupervision,
            "workspace generation build cancelled",
        )),
        Some(Ok(completion)) => completion,
        Some(Err(error)) => Err(WorkspaceGenerationBuildFailure::new(
            WorkspaceGenerationFailureStage::GenerationBuilderSupervision,
            format!("workspace generation builder task terminated before a receipt: {error}"),
        )),
        None => Err(WorkspaceGenerationBuildFailure::new(
            WorkspaceGenerationFailureStage::GenerationBuilderSupervision,
            "workspace generation builder supervisor lost its task",
        )),
    }
}

#[cfg(test)]
#[path = "../tests/unit/runtime_server_admission_builder_supervisor.rs"]
mod tests;
