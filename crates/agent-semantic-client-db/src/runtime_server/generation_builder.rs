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

enum MutationOwnerProjection {
    Owner(crate::runtime_server_workspace::WorkspaceOwnerSnapshot),
    Tombstone(String),
}

async fn project_mutation_owner(
    owner_projection_builder: &crate::runtime_server_admission::WorkspaceOwnerProjectionBuilder,
    workspace_identity: &str,
    project_root: &std::path::Path,
    changed_path: &std::path::Path,
) -> Result<MutationOwnerProjection, WorkspaceGenerationBuildFailure> {
    let owner_path = changed_path
        .strip_prefix(project_root)
        .map_err(|_| {
            WorkspaceGenerationBuildFailure::new(
                crate::runtime_server_admission::WorkspaceGenerationFailureStage::SourceBuilder,
                format!(
                    "changed owner is outside Runtime workspace: workspace={} owner={}",
                    project_root.display(),
                    changed_path.display()
                ),
            )
        })?
        .to_string_lossy()
        .replace('\\', "/");
    let exists = tokio::fs::try_exists(changed_path).await.map_err(|error| {
        WorkspaceGenerationBuildFailure::new(
            crate::runtime_server_admission::WorkspaceGenerationFailureStage::SourceBuilder,
            format!("failed to inspect changed owner: owner={owner_path} error={error}"),
        )
    })?;
    if !exists {
        return Ok(MutationOwnerProjection::Tombstone(owner_path));
    }
    owner_projection_builder(
        workspace_identity.to_owned(),
        project_root.to_path_buf(),
        owner_path,
    )
    .await
    .map(MutationOwnerProjection::Owner)
    .map_err(|error| {
        WorkspaceGenerationBuildFailure::new(
            crate::runtime_server_admission::WorkspaceGenerationFailureStage::SourceBuilder,
            error,
        )
    })
}

async fn project_mutation_owners(
    owner_projection_builder: &crate::runtime_server_admission::WorkspaceOwnerProjectionBuilder,
    workspace_identity: &str,
    project_root: &std::path::Path,
    changed_paths: &std::collections::BTreeSet<std::path::PathBuf>,
) -> Result<
    (
        Vec<crate::runtime_server_workspace::WorkspaceOwnerSnapshot>,
        Vec<String>,
    ),
    WorkspaceGenerationBuildFailure,
> {
    use tokio_stream::StreamExt;

    tokio_stream::iter(changed_paths)
        .then(|changed_path| {
            project_mutation_owner(
                owner_projection_builder,
                workspace_identity,
                project_root,
                changed_path,
            )
        })
        .collect::<Vec<_>>()
        .await
        .into_iter()
        .try_fold((Vec::new(), Vec::new()), |mut projected, item| {
            match item? {
                MutationOwnerProjection::Owner(owner) => projected.0.push(owner),
                MutationOwnerProjection::Tombstone(owner_path) => projected.1.push(owner_path),
            }
            Ok(projected)
        })
}

pub(crate) async fn publish_mutation_generation(
    owner_projection_builder: &crate::runtime_server_admission::WorkspaceOwnerProjectionBuilder,
    memory_registry: &std::sync::Arc<
        crate::runtime_server_workspace::RuntimeServerWorkspaceRegistry,
    >,
    workspace_identity: &str,
    project_root: &std::path::Path,
    changed_paths: &std::collections::BTreeSet<std::path::PathBuf>,
    request_id: String,
    mut candidate: crate::runtime_server_admission::WorkspaceGenerationCandidateIdentity,
) -> Result<
    crate::runtime_server_admission::WorkspaceGenerationBuildCompletion,
    WorkspaceGenerationBuildFailure,
> {
    let (owners, tombstones) = project_mutation_owners(
        owner_projection_builder,
        workspace_identity,
        project_root,
        changed_paths,
    )
    .await?;
    let base_generation_digest = memory_registry
        .lease(workspace_identity, project_root)
        .map_err(|error| WorkspaceGenerationBuildFailure::new(
            crate::runtime_server_admission::WorkspaceGenerationFailureStage::CanonicalGenerationPublication,
            error,
        ))?
        .generation()
        .generation_digest
        .clone();
    let published = memory_registry
        .publish_owner_delta(
            request_id,
            workspace_identity,
            project_root,
            crate::runtime_server_workspace::WorkspaceGenerationDelta {
                schema_id: crate::runtime_server_workspace::WORKSPACE_GENERATION_DELTA_SCHEMA_ID.to_owned(),
                schema_version: "1".to_owned(),
                base_generation_digest,
                owners,
                tombstones,
            },
        )
        .await
        .map_err(|error| {
            WorkspaceGenerationBuildFailure::new(
                crate::runtime_server_admission::WorkspaceGenerationFailureStage::CanonicalGenerationPublication,
                error,
            )
        })?;
    let mut candidate_digest = blake3::Hasher::new();
    candidate_digest.update(b"workspace-mutation-candidate-v1\0");
    candidate_digest.update(published.source_root_digest.as_bytes());
    candidate.candidate_generation.digest = format!("blake3:{}", candidate_digest.finalize());
    let commit = crate::runtime_server_admission::WorkspaceGenerationCommitReceipt::from_recovery(
        &published,
    )
    .map_err(|error| {
        WorkspaceGenerationBuildFailure::new(
            crate::runtime_server_admission::WorkspaceGenerationFailureStage::CanonicalGenerationPublication,
            error,
        )
    })?;
    crate::runtime_server_admission::WorkspaceGenerationBuildCompletion::new(candidate, commit)
        .map_err(|error| {
            WorkspaceGenerationBuildFailure::new(
                crate::runtime_server_admission::WorkspaceGenerationFailureStage::CanonicalGenerationPublication,
                error,
            )
        })
}

#[cfg(test)]
#[path = "../../tests/unit/runtime_server_generation_builder_mutation.rs"]
mod mutation_tests;
