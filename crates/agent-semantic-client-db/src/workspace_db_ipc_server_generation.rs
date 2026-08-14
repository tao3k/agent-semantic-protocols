use std::path::{Path, PathBuf};

use crate::runtime_server_admission::WorkspaceGenerationAdmission;
use crate::runtime_server_workspace::RuntimeServerWorkspaceRegistry;
use crate::workspace_db_ipc::WorkspaceDbIpcResult;

const GENERATION_DISCOVERY_SCHEMA_ID: &str =
    "agent.semantic-protocols.runtime-server-workspace-generation-discovery.v1";

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum WorkspaceGenerationDiscoveryState {
    Discovering,
    Failed,
    Cancelled,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceGenerationDiscoveryReceipt {
    pub schema_id: String,
    pub schema_version: String,
    pub workspace_identity: String,
    pub project_root: String,
    pub state: WorkspaceGenerationDiscoveryState,
    pub attempt: u64,
    pub started_at_unix_ms: u64,
    pub finished_at_unix_ms: Option<u64>,
    pub reason_kind: Option<String>,
    pub error: Option<String>,
}

static GENERATION_DISCOVERY_RECEIPTS: std::sync::LazyLock<
    dashmap::DashMap<GenerationDiscoveryKey, WorkspaceGenerationDiscoveryReceipt>,
> = std::sync::LazyLock::new(dashmap::DashMap::new);

fn unix_time_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or_default()
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
struct SubmittedMutationKey {
    workspace_identity: String,
    project_root: String,
    mutation_id: String,
}

static SUBMITTED_MUTATIONS: std::sync::LazyLock<dashmap::DashSet<SubmittedMutationKey>> =
    std::sync::LazyLock::new(dashmap::DashSet::new);

struct SubmittedMutationGuard(SubmittedMutationKey);

impl Drop for SubmittedMutationGuard {
    fn drop(&mut self) {
        SUBMITTED_MUTATIONS.remove(&self.0);
    }
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
struct GenerationDiscoveryKey {
    workspace_identity: String,
    project_root: PathBuf,
}

static GENERATION_DISCOVERY_TASKS: std::sync::LazyLock<dashmap::DashSet<GenerationDiscoveryKey>> =
    std::sync::LazyLock::new(dashmap::DashSet::new);

/// Completion is separate from admission state: candidate discovery can fail
/// before it creates a Building receipt.  Every waiting search/query caller
/// therefore observes the one task completion and then reads the authoritative
/// admission terminal state, rather than polling or launching another scan.
static GENERATION_DISCOVERY_COMPLETIONS: std::sync::LazyLock<
    dashmap::DashMap<GenerationDiscoveryKey, tokio::sync::watch::Sender<bool>>,
> = std::sync::LazyLock::new(dashmap::DashMap::new);

fn generation_is_active(
    state: &crate::runtime_server_admission::WorkspaceGenerationAdmissionState,
) -> bool {
    match state {
        crate::runtime_server_admission::WorkspaceGenerationAdmissionState::Building
        | crate::runtime_server_admission::WorkspaceGenerationAdmissionState::Ready => true,
        crate::runtime_server_admission::WorkspaceGenerationAdmissionState::Failed
        | crate::runtime_server_admission::WorkspaceGenerationAdmissionState::Cancelled => false,
    }
}

pub(super) fn require_terminal_generation_for_read(
    generation_admission: Option<&WorkspaceGenerationAdmission>,
    workspace_identity: &str,
    project_root: &Path,
) -> Result<(), String> {
    let Some(receipt) = generation_admission
        .and_then(|admission| admission.current(workspace_identity, project_root))
    else {
        return Ok(());
    };
    if receipt.state == crate::runtime_server_admission::WorkspaceGenerationAdmissionState::Ready
        && receipt.commit.is_some()
    {
        return Ok(());
    }
    Err(format!(
        "active workspace generation is required before resident read: workspaceIdentity={workspace_identity} state={:?} reasonKind=active-workspace-generation-required",
        receipt.state
    ))
}

pub(super) fn resident_read_project_root(
    operation: &crate::workspace_db_ipc::WorkspaceDbIpcOperation,
) -> Option<&Path> {
    let project_root = match operation {
        crate::workspace_db_ipc::WorkspaceDbIpcOperation::ReadRuntimeSelector {
            project_root,
            ..
        }
        | crate::workspace_db_ipc::WorkspaceDbIpcOperation::ReadRuntimeOwner {
            project_root,
            ..
        }
        | crate::workspace_db_ipc::WorkspaceDbIpcOperation::ReadRuntimeSearchGenerationAuthority {
            project_root,
        }
        | crate::workspace_db_ipc::WorkspaceDbIpcOperation::ReadRuntimeGraphFacts {
            project_root,
            ..
        }
        | crate::workspace_db_ipc::WorkspaceDbIpcOperation::EvaluateGraphTurbo {
            project_root,
            ..
        } => project_root,
        crate::workspace_db_ipc::WorkspaceDbIpcOperation::ReadSourceIndex { request } => {
            return Some(request.project_root.as_path());
        }
        _ => return None,
    };
    Some(Path::new(project_root))
}

struct GenerationDiscoveryTaskGuard {
    key: GenerationDiscoveryKey,
    finished: bool,
}

impl Drop for GenerationDiscoveryTaskGuard {
    fn drop(&mut self) {
        GENERATION_DISCOVERY_TASKS.remove(&self.key);
        GENERATION_DISCOVERY_COMPLETIONS.remove(&self.key);
        if !self.finished {
            if let Some(mut receipt) = GENERATION_DISCOVERY_RECEIPTS.get_mut(&self.key) {
                receipt.state = WorkspaceGenerationDiscoveryState::Cancelled;
                receipt.finished_at_unix_ms = Some(unix_time_ms());
                receipt.reason_kind = Some("discovery-cancelled".to_owned());
                receipt.error = Some("workspace candidate discovery task was cancelled".to_owned());
            }
        }
    }
}

fn schedule_single_flight_discovery(
    admission: std::sync::Arc<WorkspaceGenerationAdmission>,
    workspace_identity: &str,
    project_root: &std::path::Path,
) -> tokio::sync::watch::Receiver<bool> {
    let key = GenerationDiscoveryKey {
        workspace_identity: workspace_identity.to_owned(),
        project_root: project_root.to_path_buf(),
    };
    if let Some(receipt) = admission.current(workspace_identity, project_root)
        && generation_is_active(&receipt.state)
    {
        if let Some(completion) = GENERATION_DISCOVERY_COMPLETIONS.get(&key) {
            return completion.subscribe();
        }
        // A Ready/Building receipt may have been admitted by the mutation or
        // restore lane rather than discovery.  It already has its own
        // admission watch; never fabricate a second source scan here.
        let (completion, receiver) = tokio::sync::watch::channel(true);
        drop(completion);
        return receiver;
    }
    if !GENERATION_DISCOVERY_TASKS.insert(key.clone()) {
        return GENERATION_DISCOVERY_COMPLETIONS
            .get(&key)
            .expect("existing discovery owns a completion sender")
            .subscribe();
    }
    let (completion, completion_receiver) = tokio::sync::watch::channel(false);
    GENERATION_DISCOVERY_COMPLETIONS.insert(key.clone(), completion.clone());
    let started_at_unix_ms = unix_time_ms();
    GENERATION_DISCOVERY_RECEIPTS.insert(
        key.clone(),
        WorkspaceGenerationDiscoveryReceipt {
            schema_id: GENERATION_DISCOVERY_SCHEMA_ID.to_owned(),
            schema_version: "1".to_owned(),
            workspace_identity: key.workspace_identity.clone(),
            project_root: key.project_root.display().to_string(),
            state: WorkspaceGenerationDiscoveryState::Discovering,
            attempt: 1,
            started_at_unix_ms,
            finished_at_unix_ms: None,
            reason_kind: None,
            error: None,
        },
    );
    let task_admission = std::sync::Arc::clone(&admission);
    let task = tokio::spawn(async move {
        let mut guard = GenerationDiscoveryTaskGuard {
            key,
            finished: false,
        };
        let result = run_discovery(
            task_admission,
            guard.key.workspace_identity.clone(),
            guard.key.project_root.clone(),
        )
        .await;
        let failure = match result {
            Ok(_) => None,
            Err(error) => Some(("discovery-failed", error)),
        };
        if let Some((reason_kind, error)) = failure {
            if let Some(mut receipt) = GENERATION_DISCOVERY_RECEIPTS.get_mut(&guard.key) {
                receipt.state = WorkspaceGenerationDiscoveryState::Failed;
                receipt.finished_at_unix_ms = Some(unix_time_ms());
                receipt.reason_kind = Some(reason_kind.to_owned());
                receipt.error = Some(error.clone());
            }
            eprintln!(
                "[runtime-server-generation-discovery] workspaceIdentity={} projectRoot={} state=failed error={error}",
                guard.key.workspace_identity,
                guard.key.project_root.display()
            );
        }
        guard.finished = true;
        completion.send_replace(true);
    });
    admission.track_submission_task(task);
    completion_receiver
}

async fn run_discovery(
    admission: std::sync::Arc<WorkspaceGenerationAdmission>,
    workspace_identity: String,
    project_root: PathBuf,
) -> Result<crate::runtime_server_admission::WorkspaceGenerationAdmissionReceipt, String> {
    let candidate =
        crate::runtime_server_admission::discover_workspace_generation_candidate(&project_root)
            .await?;
    admission
        .ensure(&workspace_identity, &project_root, candidate)
        .await
}

#[cfg(test)]
#[path = "../tests/unit/workspace_db_ipc_server_generation_discovery.rs"]
mod discovery_task_tests;

#[cfg(test)]
#[path = "../tests/unit/workspace_db_ipc_server_generation_mutation.rs"]
mod mutation_submission_tests;

pub(super) fn submit_mutation(
    memory_registry: std::sync::Arc<RuntimeServerWorkspaceRegistry>,
    generation_admission: Option<std::sync::Arc<WorkspaceGenerationAdmission>>,
    workspace_identity: String,
    mutation_id: String,
    project_root: String,
    changed_paths: Vec<String>,
) -> WorkspaceDbIpcResult {
    if mutation_id.trim().is_empty() || changed_paths.is_empty() {
        return WorkspaceDbIpcResult::Failed {
            code: "runtime-server-generation-submission-invalid".to_owned(),
            message: "runtime generation submission requires a mutation id and changed paths"
                .to_owned(),
        };
    }
    let Some(generation_admission) = generation_admission else {
        return WorkspaceDbIpcResult::Failed {
            code: "runtime-server-generation-admission-unavailable".to_owned(),
            message: "Runtime Server has no canonical generation builder".to_owned(),
        };
    };
    let key = SubmittedMutationKey {
        workspace_identity: workspace_identity.clone(),
        project_root: project_root.clone(),
        mutation_id: mutation_id.clone(),
    };
    let queued = SUBMITTED_MUTATIONS.insert(key.clone());
    let receipt =
        crate::runtime_server_admission::WorkspaceGenerationMutationSubmissionReceipt {
            schema_id: crate::runtime_server_admission::WORKSPACE_GENERATION_MUTATION_SUBMISSION_RECEIPT_SCHEMA_ID.to_owned(),
            schema_version: "1".to_owned(),
            mutation_id: mutation_id.clone(),
            workspace_identity: workspace_identity.clone(),
            changed_path_count: changed_paths.len(),
            state: if queued {
                crate::runtime_server_admission::WorkspaceGenerationMutationSubmissionState::Queued
            } else {
                crate::runtime_server_admission::WorkspaceGenerationMutationSubmissionState::Coalesced
            },
        };
    if let Err(message) = receipt.validate() {
        return WorkspaceDbIpcResult::Failed {
            code: "runtime-server-generation-submission-invalid".to_owned(),
            message,
        };
    }
    if queued {
        let admission_task = std::sync::Arc::clone(&generation_admission);
        let task = tokio::spawn(async move {
            let _submission = SubmittedMutationGuard(key);
            let project_root_path = std::path::Path::new(&project_root);
            let initial_publication = if admission_task
                .current(&workspace_identity, project_root_path)
                .is_none()
            {
                Some(
                    publish_lifecycle_generation(
                        &memory_registry,
                        Some(std::sync::Arc::clone(&admission_task)),
                        &workspace_identity,
                        project_root.clone(),
                    )
                    .await,
                )
            } else {
                None
            };
            let result = match initial_publication {
                Some(Err(message)) => WorkspaceDbIpcResult::Failed {
                    code: "runtime-server-lifecycle-generation-publication-failed".to_owned(),
                    message,
                },
                Some(Ok(())) | None => {
                    match admission_task.current(&workspace_identity, project_root_path) {
                        Some(current) => {
                            let candidate = crate::runtime_server_admission::WorkspaceGenerationCandidateIdentity {
                                candidate_generation: current.candidate_generation,
                                policy_overlay_digest: current.policy_overlay_digest,
                            };
                            admit_mutation(
                                &memory_registry,
                                Some(&admission_task),
                                &workspace_identity,
                                mutation_id,
                                project_root,
                                changed_paths,
                                candidate,
                            )
                            .await
                        }
                        None => WorkspaceDbIpcResult::Failed {
                            code: "runtime-server-generation-not-admitted".to_owned(),
                            message: format!(
                                "runtime generation mutation discovery completed without an admitted resident generation: workspaceIdentity={workspace_identity} projectRoot={project_root}"
                            ),
                        },
                    }
                }
            };
            if let WorkspaceDbIpcResult::Failed { code, message } = result {
                eprintln!(
                    "[runtime-server-generation-submission] status=failed code={code} error={message}"
                );
            }
        });
        generation_admission.track_submission_task(task);
    }
    WorkspaceDbIpcResult::RuntimeGenerationMutationSubmission { receipt }
}

pub(super) async fn admit_mutation(
    memory_registry: &RuntimeServerWorkspaceRegistry,
    generation_admission: Option<&WorkspaceGenerationAdmission>,
    workspace_identity: &str,
    mutation_id: String,
    project_root: String,
    changed_paths: Vec<String>,
    candidate: crate::runtime_server_admission::WorkspaceGenerationCandidateIdentity,
) -> WorkspaceDbIpcResult {
    let project_root = PathBuf::from(project_root);
    let changed_paths = changed_paths
        .into_iter()
        .map(PathBuf::from)
        .map(|path| {
            if path.is_absolute() {
                path
            } else {
                project_root.join(path)
            }
        })
        .collect::<Vec<_>>();
    let identity = memory_registry
        .publish_owner_identity_delta(
            mutation_id.clone(),
            workspace_identity,
            &project_root,
            &changed_paths,
        )
        .await;
    match (identity, generation_admission) {
        (Ok(()), Some(admission)) => match admission
            .admit_observed_mutation_terminal(
                mutation_id,
                workspace_identity.to_owned(),
                project_root,
                changed_paths,
                candidate,
            )
            .await
        {
            Ok(receipt) => WorkspaceDbIpcResult::RuntimeGenerationMutationAdmission { receipt },
            Err(message) => WorkspaceDbIpcResult::Failed {
                code: "runtime-server-generation-admission-failed".to_owned(),
                message,
            },
        },
        (Ok(()), None) => WorkspaceDbIpcResult::Failed {
            code: "runtime-server-generation-admission-unavailable".to_owned(),
            message: "Runtime Server has no canonical generation builder".to_owned(),
        },
        (Err(message), _) => WorkspaceDbIpcResult::Failed {
            code: "runtime-server-owner-identity-publication-failed".to_owned(),
            message,
        },
    }
}

async fn publish_lifecycle_generation(
    memory_registry: &RuntimeServerWorkspaceRegistry,
    generation_admission: Option<std::sync::Arc<WorkspaceGenerationAdmission>>,
    workspace_identity: &str,
    project_root: String,
) -> Result<(), String> {
    let Some(admission) = generation_admission else {
        return Err("Runtime Server has no canonical generation builder".to_owned());
    };
    let project_root = PathBuf::from(project_root);
    let mut receipt = match admission.current(workspace_identity, &project_root) {
        Some(receipt) if generation_is_active(&receipt.state) => receipt,
        _ => {
            let mut completion = schedule_single_flight_discovery(
                std::sync::Arc::clone(&admission),
                workspace_identity,
                &project_root,
            );
            if !*completion.borrow() {
                if completion.changed().await.is_err() {
                    return Err(
                        "Runtime Server lifecycle generation discovery channel closed".to_owned(),
                    );
                }
            }
            match admission
                .wait_terminal(workspace_identity, &project_root)
                .await
            {
                Ok(receipt) => receipt,
                Err(message) => return Err(message),
            }
        }
    };
    if receipt.state == crate::runtime_server_admission::WorkspaceGenerationAdmissionState::Building
    {
        receipt = match admission
            .wait_terminal(workspace_identity, &project_root)
            .await
        {
            Ok(receipt) => receipt,
            Err(message) => return Err(message),
        };
    }
    async {
        let terminal = receipt;
        terminal.validate()?;
        if terminal.state
            != crate::runtime_server_admission::WorkspaceGenerationAdmissionState::Ready
        {
            return Err(format!(
                "runtime generation terminal admission is not ready: workspaceIdentity={} projectRoot={} state={:?} error={}",
                workspace_identity,
                project_root.display(),
                terminal.state,
                terminal.error.as_deref().unwrap_or("none"),
            ));
        }
        let admission_commit = terminal
            .commit
            .as_ref()
            .ok_or_else(|| "terminal Ready admission omitted generation commit".to_owned())?;
        let commit = match memory_registry
            .published_generation_state(workspace_identity, &project_root)
            .await?
        {
            crate::runtime_server_workspace::PublishedWorkspaceGenerationState::Ready => {
                let generation = memory_registry.lease(workspace_identity, &project_root)?;
                crate::runtime_server_admission::WorkspaceGenerationCommitReceipt {
                    active_epoch: generation.epoch(),
                    generation_digest: generation.runtime_generation_digest(),
                    source_root_digest: generation
                        .generation()
                        .source_snapshot
                        .root_digest
                        .clone(),
                }
            }
            crate::runtime_server_workspace::PublishedWorkspaceGenerationState::Missing
            | crate::runtime_server_workspace::PublishedWorkspaceGenerationState::RecoveryRequired { .. } => {
                let restored = memory_registry
                    .restore_published_generation(
                        format!("lifecycle-generation-publication-{workspace_identity}"),
                        workspace_identity.to_owned(),
                        &project_root,
                    )
                    .await?;
                crate::runtime_server_admission::WorkspaceGenerationCommitReceipt::from_recovery(
                    &restored,
                )?
            }
        };
        if commit.generation_digest != admission_commit.generation_digest
            || commit.source_root_digest != admission_commit.source_root_digest
        {
            return Err(format!(
                "published generation differs from terminal admission: workspaceIdentity={} admissionGeneration={} publishedGeneration={} admissionRoot={} publishedRoot={}",
                workspace_identity,
                admission_commit.generation_digest,
                commit.generation_digest,
                admission_commit.source_root_digest,
                commit.source_root_digest,
            ));
        }
        Ok(())
    }
    .await
}
