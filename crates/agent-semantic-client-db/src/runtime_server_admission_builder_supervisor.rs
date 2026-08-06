//! Tokio supervision boundary for one workspace generation builder.

use std::path::PathBuf;

use super::runtime_server_admission::{
    WorkspaceGenerationBuildCompletion, WorkspaceGenerationBuildFailure,
    WorkspaceGenerationBuildMode, WorkspaceGenerationBuilder, WorkspaceGenerationCandidateIdentity,
    WorkspaceGenerationFailureStage,
};

pub(super) const WORKSPACE_GENERATION_BUILD_DEADLINE: std::time::Duration =
    std::time::Duration::from_millis(800);

pub(super) async fn run(
    builder: WorkspaceGenerationBuilder,
    workspace_identity: String,
    project_root: PathBuf,
    candidate: WorkspaceGenerationCandidateIdentity,
    build_mode: WorkspaceGenerationBuildMode,
    cancellation: crate::runtime_generation_cancellation::GenerationCancellation,
) -> Result<WorkspaceGenerationBuildCompletion, WorkspaceGenerationBuildFailure> {
    run_with_deadline(
        builder,
        workspace_identity,
        project_root,
        candidate,
        build_mode,
        cancellation,
        WORKSPACE_GENERATION_BUILD_DEADLINE,
    )
    .await
}

pub(crate) async fn run_with_deadline(
    builder: WorkspaceGenerationBuilder,
    workspace_identity: String,
    project_root: PathBuf,
    candidate: WorkspaceGenerationCandidateIdentity,
    build_mode: WorkspaceGenerationBuildMode,
    cancellation: crate::runtime_generation_cancellation::GenerationCancellation,
    deadline: std::time::Duration,
) -> Result<WorkspaceGenerationBuildCompletion, String> {
    let absolute_deadline = tokio::time::Instant::now() + deadline;
    let mut builders = tokio::task::JoinSet::new();
    builders.spawn(builder(
        workspace_identity,
        project_root,
        candidate,
        build_mode,
        cancellation.clone(),
        absolute_deadline,
    ));
    let outcome = tokio::time::timeout_at(absolute_deadline, async {
        tokio::select! {
            _ = cancellation.cancelled() => None,
            result = builders.join_next() => result,
        }
    })
    .await;
    match outcome {
        Err(_) => Err(format!(
            "workspace-generation-build-deadline-exceeded: workspace generation build exceeded {deadline:?}"
        )),
        Ok(outcome) => match outcome {
            None if cancellation.is_cancelled() => {
                Err("workspace generation build cancelled".to_owned())
            }
            Some(Ok(completion)) => completion,
            Some(Err(error)) => Err(format!(
                "workspace generation builder task terminated before a receipt: {error}"
            )),
            None => Err("workspace generation builder supervisor lost its task".to_owned()),
        },
    }
}

#[cfg(test)]
#[path = "../tests/unit/runtime_server_admission_builder_supervisor.rs"]
mod tests;
