use crate::runtime_server_admission::{
    WorkspaceGenerationBuildFailure, WorkspaceGenerationFailureStage,
};
use std::future::Future;
use std::time::Instant;
use tokio::time::Instant as TokioInstant;

#[derive(Clone, Copy)]
pub(crate) enum Stage {
    WorkspaceBootstrap,
    DurableRestore,
    SourceBuilder,
    SourceIndexCommit,
    CanonicalGenerationPublication,
}

impl Stage {
    fn label(self) -> &'static str {
        match self {
            Self::WorkspaceBootstrap => "workspace-bootstrap",
            Self::DurableRestore => "durable-restore",
            Self::SourceBuilder => "source-builder",
            Self::SourceIndexCommit => "source-index-commit",
            Self::CanonicalGenerationPublication => "canonical-generation-publication",
        }
    }

    fn failure_stage(self) -> WorkspaceGenerationFailureStage {
        match self {
            Self::WorkspaceBootstrap => WorkspaceGenerationFailureStage::WorkspaceBootstrap,
            Self::DurableRestore => WorkspaceGenerationFailureStage::DurableRestore,
            Self::SourceBuilder => WorkspaceGenerationFailureStage::SourceBuilder,
            Self::SourceIndexCommit => WorkspaceGenerationFailureStage::SourceIndexCommit,
            Self::CanonicalGenerationPublication => {
                WorkspaceGenerationFailureStage::CanonicalGenerationPublication
            }
        }
    }
}

pub(crate) async fn await_stage<T, F>(
    started: Instant,
    deadline: TokioInstant,
    stage: Stage,
    future: F,
) -> Result<T, WorkspaceGenerationBuildFailure>
where
    F: Future<Output = Result<T, WorkspaceGenerationBuildFailure>>,
{
    let remaining = deadline.saturating_duration_since(TokioInstant::now());
    if remaining.is_zero() {
        return Err(WorkspaceGenerationBuildFailure::new(
            stage.failure_stage(),
            format!(
                "deadline exceeded elapsedMicros={}",
                started.elapsed().as_micros()
            ),
        ));
    }
    match tokio::time::timeout_at(deadline, future).await {
        Ok(result) => result,
        Err(_) => Err(WorkspaceGenerationBuildFailure::new(
            stage.failure_stage(),
            format!(
                "deadline exceeded elapsedMicros={}",
                started.elapsed().as_micros()
            ),
        )),
    }
}
