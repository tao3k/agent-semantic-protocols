//! Repository-candidate identity and builder contracts for workspace admission.

use std::future::Future;
use std::path::PathBuf;
use std::pin::Pin;
use std::sync::Arc;

use serde::{Deserialize, Serialize};

use super::WorkspaceGenerationCommitReceipt;

impl agent_semantic_runtime::git::CancellationProbe
    for crate::runtime_generation_cancellation::GenerationCancellation
{
    fn is_cancelled(&self) -> bool {
        self.is_cancelled()
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WorkspaceGenerationCandidateIdentity {
    pub candidate_generation: agent_semantic_runtime::git::RepositoryCandidateGeneration,
    pub policy_overlay_digest: String,
}

impl WorkspaceGenerationCandidateIdentity {
    pub fn from_snapshot(
        snapshot: &agent_semantic_runtime::git::RepositoryCandidateSnapshot,
    ) -> Self {
        Self {
            candidate_generation: snapshot.candidate_generation.clone(),
            policy_overlay_digest: snapshot.policy_overlay_digest.clone(),
        }
    }

    pub fn validate(&self) -> Result<(), String> {
        let candidate_digest = self
            .candidate_generation
            .digest
            .strip_prefix("blake3:")
            .filter(|digest| {
                digest.len() == 64 && digest.bytes().all(|byte| byte.is_ascii_hexdigit())
            });
        let policy_digest = self
            .policy_overlay_digest
            .strip_prefix("blake3:")
            .filter(|digest| {
                digest.len() == 64 && digest.bytes().all(|byte| byte.is_ascii_hexdigit())
            });
        if self.candidate_generation.algorithm != "blake3-worktree-state-v1"
            || candidate_digest.is_none()
            || self.candidate_generation.authorities.is_empty()
            || policy_digest.is_none()
        {
            return Err("workspace generation candidate identity is incomplete".to_owned());
        }
        Ok(())
    }
}

pub async fn discover_workspace_generation_candidate(
    project_root: &std::path::Path,
) -> Result<WorkspaceGenerationCandidateIdentity, String> {
    discover_workspace_generation_candidate_with_cancellation(
        project_root,
        &crate::runtime_generation_cancellation::GenerationCancellation::new(),
    )
    .await
}

pub async fn discover_workspace_generation_candidate_with_cancellation(
    project_root: &std::path::Path,
    cancellation: &crate::runtime_generation_cancellation::GenerationCancellation,
) -> Result<WorkspaceGenerationCandidateIdentity, String> {
    if cancellation.is_cancelled() {
        return Err("generation build cancelled".to_owned());
    }
    let project_root = project_root.to_path_buf();
    let cancellation = cancellation.clone();
    tokio::task::spawn_blocking(move || {
        agent_semantic_runtime::git::discover_repository_candidate_snapshot_cancellable(
            &project_root,
            &cancellation,
        )
    })
    .await
    .map_err(|error| format!("workspace candidate discovery task failed: {error}"))?
    .map_err(|error| format!("discover workspace repository candidates: {error}"))?
    .map(|snapshot| WorkspaceGenerationCandidateIdentity::from_snapshot(&snapshot))
    .ok_or_else(|| "workspace generation admission requires a Git candidate snapshot".to_owned())
}

pub struct WorkspaceGenerationBuild {
    pub candidate: WorkspaceGenerationCandidateIdentity,
    pub refresh: crate::ClientDbSourceIndexRefreshRequest,
    pub materialization: crate::runtime_server_workspace::WorkspaceCanonicalMaterialization,
}

impl WorkspaceGenerationBuild {
    pub fn new(
        candidate: WorkspaceGenerationCandidateIdentity,
        refresh: crate::ClientDbSourceIndexRefreshRequest,
        materialization: crate::runtime_server_workspace::WorkspaceCanonicalMaterialization,
    ) -> Self {
        Self {
            candidate,
            refresh,
            materialization,
        }
    }
}

pub type WorkspaceGenerationBuildFuture = Pin<
    Box<
        dyn Future<
                Output = Result<
                    WorkspaceGenerationBuildCompletion,
                    WorkspaceGenerationBuildFailure,
                >,
            > + Send
            + 'static,
    >,
>;

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum WorkspaceGenerationFailureStage {
    GenerationBuilder,
    GenerationBuilderSupervision,
    WorkspaceBootstrap,
    DurableRestore,
    SourceBuilder,
    SourceIndexCommit,
    CanonicalGenerationPublication,
    AdmissionValidation,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct WorkspaceGenerationBuildFailure {
    pub stage: WorkspaceGenerationFailureStage,
    pub message: String,
}

impl WorkspaceGenerationBuildFailure {
    pub fn new(stage: WorkspaceGenerationFailureStage, message: impl Into<String>) -> Self {
        Self {
            stage,
            message: message.into(),
        }
    }
}

impl std::fmt::Display for WorkspaceGenerationBuildFailure {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "{}: {}",
            serde_json::to_string(&self.stage).unwrap_or_default(),
            self.message
        )
    }
}

impl std::error::Error for WorkspaceGenerationBuildFailure {}

pub struct WorkspaceGenerationBuildCompletion {
    pub candidate: WorkspaceGenerationCandidateIdentity,
    pub commit: WorkspaceGenerationCommitReceipt,
}

impl WorkspaceGenerationBuildCompletion {
    pub fn new(
        candidate: WorkspaceGenerationCandidateIdentity,
        commit: WorkspaceGenerationCommitReceipt,
    ) -> Result<Self, String> {
        candidate.validate()?;
        commit.validate()?;
        Ok(Self { candidate, commit })
    }
}
pub type WorkspaceGenerationBuilder = Arc<
    dyn Fn(
            String,
            PathBuf,
            WorkspaceGenerationCandidateIdentity,
            WorkspaceGenerationBuildMode,
            crate::runtime_generation_cancellation::GenerationCancellation,
            tokio::time::Instant,
        ) -> WorkspaceGenerationBuildFuture
        + Send
        + Sync
        + 'static,
>;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WorkspaceGenerationBuildMode {
    RestoreOnly,
    RestoreOrBuild,
    RebuildAfterMutation,
}

impl WorkspaceGenerationBuildMode {
    /// Whether admission may decode and reuse the durable canonical generation.
    ///
    /// A mutation rebuild has already rejected that generation as superseded,
    /// so decoding its potentially large materialization would be both wasted
    /// work and avoidable peak memory pressure.
    pub const fn attempts_durable_restore(self) -> bool {
        !matches!(self, Self::RebuildAfterMutation)
    }
}

pub type WorkspaceGenerationCandidateBuildFuture =
    Pin<Box<dyn Future<Output = Result<WorkspaceGenerationBuild, String>> + Send + 'static>>;
pub type WorkspaceGenerationCandidateBuilder =
    Arc<dyn Fn(String, PathBuf) -> WorkspaceGenerationCandidateBuildFuture + Send + Sync + 'static>;

pub type WorkspaceOwnerProjectionBuildFuture = Pin<
    Box<
        dyn Future<Output = Result<crate::runtime_server_workspace::WorkspaceOwnerSnapshot, String>>
            + Send
            + 'static,
    >,
>;
pub type WorkspaceOwnerProjectionBuilder = Arc<
    dyn Fn(String, PathBuf, String, String) -> WorkspaceOwnerProjectionBuildFuture
        + Send
        + Sync
        + 'static,
>;
