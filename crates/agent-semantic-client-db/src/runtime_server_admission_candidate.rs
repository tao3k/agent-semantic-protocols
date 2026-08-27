//! Repository-candidate identity and builder contracts for workspace admission.

use std::future::Future;
use std::path::PathBuf;
use std::pin::Pin;
use std::sync::Arc;
use std::sync::OnceLock;

use serde::{Deserialize, Serialize};

use super::WorkspaceGenerationCommitReceipt;

type CachedCandidate = Result<WorkspaceGenerationCandidateIdentity, String>;
type CandidateCell = Arc<tokio::sync::OnceCell<CachedCandidate>>;

static CANDIDATE_CACHE: OnceLock<dashmap::DashMap<PathBuf, CandidateCell>> = OnceLock::new();

fn candidate_cache() -> &'static dashmap::DashMap<PathBuf, CandidateCell> {
    CANDIDATE_CACHE.get_or_init(dashmap::DashMap::new)
}

pub fn record_workspace_generation_candidate(
    project_root: PathBuf,
    candidate: WorkspaceGenerationCandidateIdentity,
) -> Result<(), String> {
    candidate.validate()?;
    // A completed generation is immutable, but it is not a valid input cache
    // for the next generation: the workspace may have changed before the next
    // query-demand admission. Keeping it here made invalidate/rebuild reuse an
    // obsolete canonical snapshot and fail with source-snapshot drift.
    //
    // Candidate discovery remains coalesced only while one discovery is in
    // flight. The Runtime Server admission owns all subsequent generation
    // serialization and publication.
    candidate_cache().remove(&project_root);
    Ok(())
}

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
    let cell = candidate_cache()
        .entry(project_root.clone())
        .or_insert_with(|| Arc::new(tokio::sync::OnceCell::new()))
        .clone();
    let discovery_root = project_root.clone();
    let resident_root = discovery_root.clone();
    let discovery_cancellation = cancellation.clone();
    let result = cell
        .get_or_init(|| async move {
            Ok(tokio::task::spawn_blocking(move || {
                agent_semantic_runtime::git::discover_repository_candidate_snapshot_cancellable(
                    &discovery_root,
                    &discovery_cancellation,
                )
            })
            .await
            .map_err(|error| format!("workspace candidate discovery task failed: {error}"))?
            .map_err(|error| format!("discover workspace repository candidates: {error}"))?
            .map(|snapshot| WorkspaceGenerationCandidateIdentity::from_snapshot(&snapshot))
            .unwrap_or_else(|| resident_non_git_candidate(&resident_root)))
        })
        .await
        .clone();
    candidate_cache().remove_if(&project_root, |_, cached| Arc::ptr_eq(cached, &cell));
    result
}

fn resident_non_git_candidate(
    project_root: &std::path::Path,
) -> WorkspaceGenerationCandidateIdentity {
    // This identity is deliberately path-scoped, not a filesystem owner scan.
    // The Server-owned generation transaction supplies source/overlay evidence;
    // a non-Git workspace must not be rejected or force owner rescans merely to
    // manufacture a Git candidate.
    let root = std::fs::canonicalize(project_root).unwrap_or_else(|_| project_root.to_path_buf());
    let root_text = root.to_string_lossy();
    let candidate_generation = format!(
        "blake3:{}",
        blake3::hash(format!("asp-runtime-server-resident-candidate-v1:{root_text}").as_bytes())
            .to_hex()
    );
    let policy_overlay_digest = format!(
        "blake3:{}",
        blake3::hash(format!("asp-runtime-server-resident-policy-v1:{root_text}").as_bytes())
            .to_hex()
    );
    WorkspaceGenerationCandidateIdentity {
        candidate_generation: agent_semantic_runtime::git::RepositoryCandidateGeneration {
            algorithm: "blake3-worktree-state-v1".to_owned(),
            digest: candidate_generation,
            authorities: vec![
                agent_semantic_runtime::git::RepositoryCandidateAuthority::ServerResident,
            ],
        },
        policy_overlay_digest,
    }
}

pub struct WorkspaceGenerationCandidateBuild {
    pub candidate: WorkspaceGenerationCandidateIdentity,
    pub refresh: crate::ClientDbSourceIndexRefreshRequest,
    pub materialization: crate::runtime_server_workspace::WorkspaceCanonicalMaterialization,
}

impl WorkspaceGenerationCandidateBuild {
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

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkspaceGenerationProviderTarget {
    pub language_id: String,
    /// Artifact publication supplies the exact provider identity. Query-demand
    /// reads may only know a language; the daemon resolves that against its
    /// admitted provider registry before constructing the source-index scope.
    pub provider_id: Option<String>,
}

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
            Arc<std::collections::BTreeSet<PathBuf>>,
            Option<WorkspaceGenerationProviderTarget>,
            crate::runtime_generation_cancellation::GenerationCancellation,
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

pub type WorkspaceGenerationCandidateBuildFuture = Pin<
    Box<dyn Future<Output = Result<WorkspaceGenerationCandidateBuild, String>> + Send + 'static>,
>;
pub type WorkspaceGenerationCandidateBuilder = Arc<
    dyn Fn(
            String,
            PathBuf,
            Arc<std::collections::BTreeSet<PathBuf>>,
            Option<WorkspaceGenerationProviderTarget>,
            crate::runtime_generation_cancellation::GenerationCancellation,
        ) -> WorkspaceGenerationCandidateBuildFuture
        + Send
        + Sync
        + 'static,
>;

pub type WorkspaceOwnerProjectionBuildFuture = Pin<
    Box<
        dyn Future<Output = Result<crate::runtime_server_workspace::WorkspaceOwnerSnapshot, String>>
            + Send
            + 'static,
    >,
>;
pub type WorkspaceOwnerProjectionBuilder = Arc<
    dyn Fn(
            String,
            PathBuf,
            String,
            crate::runtime_generation_cancellation::GenerationCancellation,
        ) -> WorkspaceOwnerProjectionBuildFuture
        + Send
        + Sync
        + 'static,
>;
