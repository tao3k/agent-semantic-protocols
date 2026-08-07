use crate::runtime_server_admission::WorkspaceGenerationBuildFailure;
use std::future::Future;

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
}

pub(crate) async fn await_stage<T, F>(
    workspace_identity: &str,
    operation_id: &str,
    stage: Stage,
    future: F,
) -> Result<T, WorkspaceGenerationBuildFailure>
where
    F: Future<Output = Result<T, WorkspaceGenerationBuildFailure>>,
{
    let started = std::time::Instant::now();
    let result = future.await;
    let mut observation = crate::runtime_server_opentelemetry::RuntimePerformanceObservation::new(
        "workspace-generation-admission",
        stage.label(),
        u64::try_from(started.elapsed().as_micros()).unwrap_or(u64::MAX),
        0,
        if result.is_ok() { "observed" } else { "failed" },
    )
    .with_operation_id(operation_id.to_owned());
    observation.workspace_identity = Some(workspace_identity.to_owned());
    if let Err(error) = &result {
        observation.failure_reason = Some(format!("{:?}", error.stage).to_lowercase());
    }
    let _ = crate::runtime_server_opentelemetry::try_record_to_active_runtime(observation);
    result
}
