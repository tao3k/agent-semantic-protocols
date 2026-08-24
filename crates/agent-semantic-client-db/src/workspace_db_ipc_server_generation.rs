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
    memory_registry: &RuntimeServerWorkspaceRegistry,
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
        && memory_registry
            .lease(workspace_identity, project_root)
            .is_ok()
        && memory_registry
            .resident_source_index_ready(workspace_identity, project_root)
            .is_ok()
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

#[cfg(test)]
pub(crate) async fn require_or_submit_terminal_generation_for_read(
    memory_registry: &RuntimeServerWorkspaceRegistry,
    generation_admission: Option<&std::sync::Arc<WorkspaceGenerationAdmission>>,
    workspace_identity: &str,
    project_root: &Path,
    target_paths: Vec<PathBuf>,
) -> Result<(), String> {
    require_or_submit_terminal_generation_for_read_with_provider(
        memory_registry,
        generation_admission,
        workspace_identity,
        project_root,
        target_paths,
        None,
    )
    .await
}

pub(crate) async fn require_or_submit_terminal_generation_for_read_with_provider(
    memory_registry: &RuntimeServerWorkspaceRegistry,
    generation_admission: Option<&std::sync::Arc<WorkspaceGenerationAdmission>>,
    workspace_identity: &str,
    project_root: &Path,
    target_paths: Vec<PathBuf>,
    provider_target: Option<crate::runtime_server_admission::WorkspaceGenerationProviderTarget>,
) -> Result<(), String> {
    let admission = generation_admission.map(std::sync::Arc::as_ref);
    let Err(message) = require_terminal_generation_for_read(
        memory_registry,
        admission,
        workspace_identity,
        project_root,
    ) else {
        return Ok(());
    };
    let Some(admission) = generation_admission else {
        return Err(
            "Runtime Server query-demand admission authority is unavailable reasonKind=runtime-generation-admission-unavailable"
                .to_owned(),
        );
    };
    if let Err(error) = admission
        .submit_query_demand_with_provider(
            workspace_identity.to_owned(),
            project_root.to_path_buf(),
            target_paths,
            provider_target,
        )
        .await
    {
        return Err(format!(
            "Runtime Server query-demand admission failed: {error} reasonKind=runtime-generation-admission-failed"
        ));
    }
    let terminal = admission
        .wait_terminal(workspace_identity, project_root)
        .await
        .map_err(|error| {
        format!(
            "{message}; Runtime Server query-demand admission failed: {error} reasonKind=active-workspace-generation-required"
        )
        })?;
    if terminal.state != crate::runtime_server_admission::WorkspaceGenerationAdmissionState::Ready
        || terminal.commit.is_none()
    {
        return Err(format!(
            "{message}; Runtime Server query-demand admission ended in state={:?} error={} reasonKind=active-workspace-generation-required",
            terminal.state,
            terminal.error.as_deref().unwrap_or("none")
        ));
    }
    require_terminal_generation_for_read(
        memory_registry,
        Some(admission.as_ref()),
        workspace_identity,
        project_root,
    )
}

pub(super) async fn require_or_submit_terminal_generation_for_operation(
    memory_registry: &RuntimeServerWorkspaceRegistry,
    generation_admission: Option<&std::sync::Arc<WorkspaceGenerationAdmission>>,
    workspace_identity: &str,
    project_root: &Path,
    operation: &crate::workspace_db_ipc::WorkspaceDbIpcOperation,
) -> Result<(), String> {
    let target_paths = resident_read_query_targets(operation, project_root).map_err(|error| {
        format!(
            "Runtime Server rejected query-demand target: {error} reasonKind=runtime-generation-admission-invalid-target"
        )
    })?;
    let provider_target = resident_read_query_provider_target(operation);
    require_or_submit_terminal_generation_for_read_with_provider(
        memory_registry,
        generation_admission,
        workspace_identity,
        project_root,
        target_paths,
        provider_target,
    )
    .await
}

pub(super) fn resident_read_query_provider_target(
    operation: &crate::workspace_db_ipc::WorkspaceDbIpcOperation,
) -> Option<crate::runtime_server_admission::WorkspaceGenerationProviderTarget> {
    match operation {
        crate::workspace_db_ipc::WorkspaceDbIpcOperation::ProviderSearch {
            language_id, ..
        } => Some(
            crate::runtime_server_admission::WorkspaceGenerationProviderTarget {
                language_id: language_id.as_str().to_owned(),
                provider_id: None,
            },
        ),
        _ => None,
    }
}

pub(super) fn resident_read_query_targets(
    operation: &crate::workspace_db_ipc::WorkspaceDbIpcOperation,
    project_root: &Path,
) -> Result<Vec<PathBuf>, String> {
    let owner_path = match operation {
        crate::workspace_db_ipc::WorkspaceDbIpcOperation::ReadRuntimeSelector {
            language_id,
            structural_selector,
            ..
        } => {
            let selector = agent_semantic_content_identity::CanonicalItemSelector::parse_root_or_exact_descendant(
                structural_selector.clone(),
            )?;
            if selector.language_id.as_str() != language_id.as_str() {
                return Err(
                    "runtime selector language does not match the query-demand language".to_owned(),
                );
            }
            Some(selector.owner_path()?.to_owned())
        }
        crate::workspace_db_ipc::WorkspaceDbIpcOperation::ReadRuntimeExactProjection {
            language_id,
            structural_selector,
            ..
        } => {
            let selector = agent_semantic_content_identity::CanonicalItemSelector::parse_root_or_exact_descendant(
                structural_selector.clone(),
            )?;
            if selector.language_id.as_str() != language_id.as_str() {
                return Err(
                    "runtime selector language does not match query-demand language".to_owned(),
                );
            }
            Some(selector.owner_path()?.to_owned())
        }
        crate::workspace_db_ipc::WorkspaceDbIpcOperation::ReadRuntimeOwner {
            owner_path, ..
        } => Some(owner_path.clone()),
        crate::workspace_db_ipc::WorkspaceDbIpcOperation::ReadRuntimeMerkleOwner { request } => {
            request.validate()?;
            Some(request.owner_path.clone())
        }
        _ => None,
    };
    let Some(owner_path) = owner_path else {
        return Ok(Vec::new());
    };
    let relative = Path::new(&owner_path);
    if relative.is_absolute()
        || relative.components().any(|component| {
            matches!(
                component,
                std::path::Component::CurDir | std::path::Component::ParentDir
            )
        })
    {
        return Err("query-demand owner path must be normalized and relative".to_owned());
    }
    Ok(vec![project_root.join(relative)])
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
        | crate::workspace_db_ipc::WorkspaceDbIpcOperation::ReadRuntimeExactProjection {
            project_root,
            ..
        }
        | crate::workspace_db_ipc::WorkspaceDbIpcOperation::ProviderSearch {
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
        crate::workspace_db_ipc::WorkspaceDbIpcOperation::ReadSourceIndex { request }
        | crate::workspace_db_ipc::WorkspaceDbIpcOperation::ReadRuntimeSourceIndex { request } => {
            return Some(request.project_root.as_path());
        }
        _ => return None,
    };
    Some(Path::new(project_root))
}

#[cfg(test)]
#[path = "../tests/unit/workspace_db_ipc_server_generation_read.rs"]
mod resident_read_tests;

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
            let result =
                match crate::runtime_server_admission::discover_workspace_generation_candidate(
                    project_root_path,
                )
                .await
                {
                    Ok(candidate) => {
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
                    Err(message) => WorkspaceDbIpcResult::Failed {
                        code: "runtime-server-generation-candidate-discovery-failed".to_owned(),
                        message,
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

pub(crate) async fn require_lifecycle_generation(
    memory_registry: &RuntimeServerWorkspaceRegistry,
    _generation_admission: Option<&std::sync::Arc<WorkspaceGenerationAdmission>>,
    workspace_identity: &str,
    project_root: String,
) -> WorkspaceDbIpcResult {
    let require_started = std::time::Instant::now();
    let project_root_path = PathBuf::from(&project_root);
    let recovery = match memory_registry.ready_recovery_receipt(
        format!("require-runtime-generation:{workspace_identity}"),
        workspace_identity,
        &project_root_path,
    ) {
        Ok(recovery) => recovery,
        Err(message) => {
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
                    "Runtime Server resident generation is not Ready: workspaceIdentity={workspace_identity} error={message} reasonKind=runtime-generation-not-ready"
                ),
            };
        }
    };
    let operation_id = format!(
        "require-runtime-generation:{workspace_identity}:{}",
        recovery.generation_digest
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
    let _ = crate::runtime_server_opentelemetry::try_record_to_active_runtime(observation);
    WorkspaceDbIpcResult::RuntimeGenerationResidentReady { receipt: recovery }
}

pub(crate) async fn restore_lifecycle_generation_from_pointer(
    memory_registry: &RuntimeServerWorkspaceRegistry,
    workspace_identity: &str,
    project_root: String,
) -> WorkspaceDbIpcResult {
    let project_root = PathBuf::from(project_root);
    match memory_registry
        .restore_published_generation(
            format!("restore-runtime-generation:{workspace_identity}"),
            workspace_identity,
            &project_root,
        )
        .await
    {
        Ok(receipt) => WorkspaceDbIpcResult::RuntimeGenerationResidentReady { receipt },
        Err(message) => WorkspaceDbIpcResult::Failed {
            code: "runtime-server-generation-pointer-restore-failed".to_owned(),
            message: format!(
                "Runtime Server generation pointer restore failed: workspaceIdentity={workspace_identity} error={message} reasonKind=runtime-generation-pointer-restore-failed"
            ),
        },
    }
}

pub(crate) async fn admit_generation_for_read(
    memory_registry: &RuntimeServerWorkspaceRegistry,
    generation_admission: Option<&std::sync::Arc<WorkspaceGenerationAdmission>>,
    workspace_identity: &str,
    project_root: String,
    language_id: String,
    provider_id: String,
) -> WorkspaceDbIpcResult {
    let project_root_path = PathBuf::from(&project_root);
    let Some(generation_admission) = generation_admission else {
        return WorkspaceDbIpcResult::Failed {
            code: "runtime-server-generation-admission-unavailable".to_owned(),
            message: format!(
                "Runtime Server generation admission authority is unavailable: workspaceIdentity={workspace_identity} reasonKind=runtime-generation-admission-unavailable"
            ),
        };
    };

    let admission_build_mode = match memory_registry
        .published_generation_state(workspace_identity, &project_root_path)
        .await
    {
        Ok(crate::runtime_server_workspace::PublishedWorkspaceGenerationState::Ready)
        | Ok(crate::runtime_server_workspace::PublishedWorkspaceGenerationState::Missing) => {
            crate::runtime_server_admission::WorkspaceGenerationBuildMode::RestoreOrBuild
        }
        Ok(
            crate::runtime_server_workspace::PublishedWorkspaceGenerationState::RecoveryRequired {
                ..
            },
        ) => crate::runtime_server_admission::WorkspaceGenerationBuildMode::RebuildAfterMutation,
        Err(error) => {
            return WorkspaceDbIpcResult::Failed {
                code: "runtime-server-generation-completeness-read-failed".to_owned(),
                message: format!(
                    "Runtime Server generation completeness read failed: workspaceIdentity={workspace_identity} error={error} reasonKind=runtime-generation-completeness-read-failed"
                ),
            };
        }
    };

    match generation_admission
        .admit_artifact_publication_with_mode_and_wait(
            workspace_identity.to_owned(),
            project_root_path,
            language_id,
            provider_id,
            admission_build_mode,
        )
        .await
    {
        Ok(receipt)
            if receipt.state
                == crate::runtime_server_admission::WorkspaceGenerationAdmissionState::Ready =>
        {
            match memory_registry
                .published_generation_state(workspace_identity, Path::new(&project_root))
                .await
            {
                Ok(crate::runtime_server_workspace::PublishedWorkspaceGenerationState::Ready) => {
                    require_lifecycle_generation(
                        memory_registry,
                        Some(generation_admission),
                        workspace_identity,
                        project_root,
                    )
                    .await
                }
                Ok(state) => WorkspaceDbIpcResult::Failed {
                    code: "runtime-server-generation-not-search-ready".to_owned(),
                    message: format!(
                        "Runtime Server generation admission completed without a complete search publication: workspaceIdentity={workspace_identity} state={state:?} reasonKind=runtime-generation-not-search-ready"
                    ),
                },
                Err(error) => WorkspaceDbIpcResult::Failed {
                    code: "runtime-server-generation-completeness-read-failed".to_owned(),
                    message: format!(
                        "Runtime Server generation completeness read failed after admission: workspaceIdentity={workspace_identity} error={error} reasonKind=runtime-generation-completeness-read-failed"
                    ),
                },
            }
        }
        Ok(receipt) => WorkspaceDbIpcResult::Failed {
            code: "runtime-server-generation-admission-failed".to_owned(),
            message: format!(
                "Runtime Server read admission did not reach Ready: workspaceIdentity={workspace_identity} state={:?} error={:?} reasonKind=runtime-generation-admission-failed",
                receipt.state, receipt.error
            ),
        },
        Err(error) => WorkspaceDbIpcResult::Failed {
            code: "runtime-server-generation-admission-failed".to_owned(),
            message: format!(
                "Runtime Server read admission failed: workspaceIdentity={workspace_identity} error={error} reasonKind=runtime-generation-admission-failed"
            ),
        },
    }
}
