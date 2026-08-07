//! Tokio supervision boundary for one workspace generation builder.

use std::path::PathBuf;

use super::runtime_server_admission::{
    WorkspaceGenerationBuildCompletion, WorkspaceGenerationBuildFailure,
    WorkspaceGenerationBuildMode, WorkspaceGenerationBuilder, WorkspaceGenerationCandidateIdentity,
    WorkspaceGenerationFailureStage,
};

pub(super) async fn run(
    builder: WorkspaceGenerationBuilder,
    workspace_identity: String,
    project_root: PathBuf,
    candidate: WorkspaceGenerationCandidateIdentity,
    build_mode: WorkspaceGenerationBuildMode,
    changed_paths: std::sync::Arc<std::collections::BTreeSet<PathBuf>>,
    cancellation: crate::runtime_generation_cancellation::GenerationCancellation,
) -> Result<WorkspaceGenerationBuildCompletion, WorkspaceGenerationBuildFailure> {
    let mut builders = tokio::task::JoinSet::new();
    builders.spawn(builder(
        workspace_identity,
        project_root,
        candidate,
        build_mode,
        changed_paths,
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
pub(crate) async fn run_with_deadline(
    builder: WorkspaceGenerationBuilder,
    workspace_identity: String,
    project_root: PathBuf,
    candidate: WorkspaceGenerationCandidateIdentity,
    build_mode: WorkspaceGenerationBuildMode,
    cancellation: crate::runtime_generation_cancellation::GenerationCancellation,
    deadline: std::time::Duration,
) -> Result<WorkspaceGenerationBuildCompletion, WorkspaceGenerationBuildFailure> {
    match tokio::time::timeout(
        deadline,
        run(
            builder,
            workspace_identity,
            project_root,
            candidate,
            build_mode,
            std::sync::Arc::new(std::collections::BTreeSet::new()),
            cancellation.clone(),
        ),
    )
    .await
    {
        Err(_) => {
            cancellation.cancel();
            Err(WorkspaceGenerationBuildFailure::new(
                WorkspaceGenerationFailureStage::GenerationBuilderSupervision,
                format!("workspace generation build exceeded {deadline:?}"),
            ))
        }
        Ok(outcome) => outcome,
    }
}

#[cfg(test)]
#[path = "../tests/unit/runtime_server_admission_builder_supervisor.rs"]
mod tests;
