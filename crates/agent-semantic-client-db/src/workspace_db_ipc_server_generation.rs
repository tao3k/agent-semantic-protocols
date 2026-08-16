use std::path::{Path, PathBuf};

use crate::runtime_server_admission::WorkspaceGenerationAdmission;
use crate::runtime_server_workspace::RuntimeServerWorkspaceRegistry;
use crate::workspace_db_ipc::WorkspaceDbIpcResult;

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

pub(super) fn require_terminal_generation_for_read(
    generation_admission: Option<&WorkspaceGenerationAdmission>,
    workspace_identity: &str,
    project_root: &Path,
) -> Result<(), String> {
    let started = std::time::Instant::now();
    let Some(receipt) = generation_admission
        .and_then(|admission| admission.current(workspace_identity, project_root))
    else {
        let error = format!(
            "active workspace generation is required before resident read: workspaceIdentity={workspace_identity} state=missing reasonKind=active-workspace-generation-required"
        );
        record_generation_read_terminal(workspace_identity, started, "not-ready", Some(&error));
        return Err(error);
    };
    if receipt.state == crate::runtime_server_admission::WorkspaceGenerationAdmissionState::Ready
        && receipt.commit.is_some()
    {
        record_generation_read_terminal(workspace_identity, started, "ready", None);
        return Ok(());
    }
    let error = format!(
        "active workspace generation is required before resident read: workspaceIdentity={workspace_identity} state={:?} error={} reasonKind=active-workspace-generation-required",
        receipt.state,
        receipt.error.as_deref().unwrap_or("none"),
    );
    record_generation_read_terminal(workspace_identity, started, "not-ready", Some(&error));
    Err(error)
}

fn record_generation_read_terminal(
    workspace_identity: &str,
    started: std::time::Instant,
    state: &str,
    failure: Option<&str>,
) {
    let mut observation = crate::runtime_server_opentelemetry::RuntimePerformanceObservation::new(
        "workspace-generation-resident-read",
        "terminal",
        u64::try_from(started.elapsed().as_micros()).unwrap_or(u64::MAX),
        1_000,
        state,
    );
    observation.workspace_identity = Some(workspace_identity.to_owned());
    observation.failure_reason = failure.map(ToOwned::to_owned);
    observation.seal_budget_failure_identity();
    let _ = crate::runtime_server_opentelemetry::try_record_to_active_runtime(observation);
}

pub(super) fn resident_read_project_root(
    operation: &crate::workspace_db_ipc::WorkspaceDbIpcOperation,
) -> Option<&Path> {
    let project_root = match operation {
        crate::workspace_db_ipc::WorkspaceDbIpcOperation::ReadRuntimeSelector {
            project_root,
            ..
        }
        | crate::workspace_db_ipc::WorkspaceDbIpcOperation::ProviderSearch {
            project_root,
            ..
        }
        | crate::workspace_db_ipc::WorkspaceDbIpcOperation::ReadRuntimeOwner {
            project_root,
            ..
        }
        | crate::workspace_db_ipc::WorkspaceDbIpcOperation::ReadRuntimeMerkleOwner {
            request:
                crate::workspace_db_ipc::RuntimeMerkleOwnerReadRequest { project_root, .. },
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
    if generation_admission
        .current(&workspace_identity, std::path::Path::new(&project_root))
        .is_none()
    {
        return WorkspaceDbIpcResult::Failed {
            code: "runtime-server-generation-not-ready".to_owned(),
            message: format!(
                "runtime generation mutation requires a prepublished resident generation: workspaceIdentity={workspace_identity} projectRoot={project_root} reasonKind=runtime-generation-not-ready"
            ),
        };
    }
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
            let result = match admission_task.current(&workspace_identity, project_root_path) {
                Some(current) => {
                    let candidate =
                        crate::runtime_server_admission::WorkspaceGenerationCandidateIdentity {
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
                    code: "runtime-server-generation-not-ready".to_owned(),
                    message: format!(
                        "runtime generation mutation lost its prepublished resident generation: workspaceIdentity={workspace_identity} projectRoot={project_root} reasonKind=runtime-generation-not-ready"
                    ),
                },
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

pub(super) async fn require_lifecycle_generation(
    _memory_registry: &RuntimeServerWorkspaceRegistry,
    generation_admission: Option<&std::sync::Arc<WorkspaceGenerationAdmission>>,
    workspace_identity: &str,
    project_root: String,
) -> WorkspaceDbIpcResult {
    let require_started = std::time::Instant::now();
    let Some(admission) = generation_admission else {
        let mut observation =
            crate::runtime_server_opentelemetry::RuntimePerformanceObservation::new(
                "workspace-generation-require",
                "terminal",
                u64::try_from(require_started.elapsed().as_micros()).unwrap_or(u64::MAX),
                1_000,
                "unavailable",
            );
        observation.workspace_identity = Some(workspace_identity.to_owned());
        observation.failure_reason = Some("runtime-generation-authority-unavailable".to_owned());
        observation.seal_budget_failure_identity();
        let _ = crate::runtime_server_opentelemetry::try_record_to_active_runtime(observation);
        return WorkspaceDbIpcResult::Failed {
            code: "runtime-server-generation-authority-unavailable".to_owned(),
            message: "Runtime Server has no resident generation authority".to_owned(),
        };
    };
    let project_root_path = PathBuf::from(&project_root);
    let Some(receipt) = admission.current(workspace_identity, &project_root_path) else {
        let mut observation =
            crate::runtime_server_opentelemetry::RuntimePerformanceObservation::new(
                "workspace-generation-require",
                "terminal",
                u64::try_from(require_started.elapsed().as_micros()).unwrap_or(u64::MAX),
                1_000,
                "not-ready",
            );
        observation.workspace_identity = Some(workspace_identity.to_owned());
        observation.failure_reason = Some("runtime-generation-not-ready".to_owned());
        observation.seal_budget_failure_identity();
        let _ = crate::runtime_server_opentelemetry::try_record_to_active_runtime(observation);
        return WorkspaceDbIpcResult::Failed {
            code: "runtime-server-generation-not-ready".to_owned(),
            message: format!(
                "Runtime Server has no prepublished resident generation: workspaceIdentity={workspace_identity} reasonKind=runtime-generation-not-ready"
            ),
        };
    };
    if receipt.state != crate::runtime_server_admission::WorkspaceGenerationAdmissionState::Ready
        || receipt.commit.is_none()
    {
        let mut observation =
            crate::runtime_server_opentelemetry::RuntimePerformanceObservation::new(
                "workspace-generation-require",
                "terminal",
                u64::try_from(require_started.elapsed().as_micros()).unwrap_or(u64::MAX),
                1_000,
                "not-ready",
            );
        observation.workspace_identity = Some(workspace_identity.to_owned());
        observation.failure_reason = Some("runtime-generation-not-ready".to_owned());
        observation.seal_budget_failure_identity();
        let _ = crate::runtime_server_opentelemetry::try_record_to_active_runtime(observation);
        return WorkspaceDbIpcResult::Failed {
            code: "runtime-server-generation-not-ready".to_owned(),
            message: format!(
                "Runtime Server generation is not Ready: workspaceIdentity={workspace_identity} state={:?} error={} reasonKind=runtime-generation-not-ready",
                receipt.state,
                receipt.error.as_deref().unwrap_or("none"),
            ),
        };
    }
    let operation_id = format!(
        "require-runtime-generation:{workspace_identity}:{}",
        receipt
            .commit
            .as_ref()
            .map(|commit| commit.generation_digest.as_str())
            .unwrap_or("missing")
    );
    let mut observation = crate::runtime_server_opentelemetry::RuntimePerformanceObservation::new(
        "workspace-generation-require",
        "terminal",
        u64::try_from(require_started.elapsed().as_micros()).unwrap_or(u64::MAX),
        1_000,
        "ready",
    )
    .with_operation_id(operation_id);
    observation.workspace_identity = Some(workspace_identity.to_owned());
    observation.seal_budget_failure_identity();
    if !crate::runtime_server_opentelemetry::try_record_to_active_runtime(observation) {
        WorkspaceDbIpcResult::Failed {
            code: "runtime-server-generation-opentelemetry-unavailable".to_owned(),
            message: "Runtime Server generation terminal OpenTelemetry is unavailable".to_owned(),
        }
    } else {
        WorkspaceDbIpcResult::RuntimeGenerationReady { receipt }
    }
}
