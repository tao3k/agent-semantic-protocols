use std::path::PathBuf;

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
    let queued = SUBMITTED_MUTATIONS.insert(key);
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
            let candidate =
                crate::runtime_server_admission::discover_workspace_generation_candidate(
                    std::path::Path::new(&project_root),
                )
                .await;
            let result = match candidate {
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
            .admit_observed_mutation(
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

pub(super) async fn ensure_current_or_build(
    generation_admission: Option<&WorkspaceGenerationAdmission>,
    workspace_identity: &str,
    project_root: String,
) -> WorkspaceDbIpcResult {
    let Some(admission) = generation_admission else {
        return WorkspaceDbIpcResult::Failed {
            code: "runtime-server-generation-admission-unavailable".to_owned(),
            message: "Runtime Server has no canonical generation builder".to_owned(),
        };
    };
    let project_root = PathBuf::from(project_root);
    if let Some(receipt) = admission.current(workspace_identity, &project_root) {
        return WorkspaceDbIpcResult::RuntimeGenerationAdmission { receipt };
    }
    let candidate = match crate::runtime_server_admission::discover_workspace_generation_candidate(
        &project_root,
    )
    .await
    {
        Ok(candidate) => candidate,
        Err(message) => {
            return WorkspaceDbIpcResult::Failed {
                code: "runtime-server-generation-candidate-discovery-failed".to_owned(),
                message,
            };
        }
    };
    match admission
        .ensure(workspace_identity, &project_root, candidate)
        .await
    {
        Ok(receipt) => WorkspaceDbIpcResult::RuntimeGenerationAdmission { receipt },
        Err(message) => WorkspaceDbIpcResult::Failed {
            code: "runtime-server-generation-ensure-failed".to_owned(),
            message,
        },
    }
}

pub(super) async fn ensure_terminal_ready(
    memory_registry: &RuntimeServerWorkspaceRegistry,
    generation_admission: Option<&WorkspaceGenerationAdmission>,
    workspace_identity: &str,
    project_root: String,
) -> WorkspaceDbIpcResult {
    let Some(admission) = generation_admission else {
        return WorkspaceDbIpcResult::Failed {
            code: "runtime-server-generation-admission-unavailable".to_owned(),
            message: "Runtime Server has no canonical generation builder".to_owned(),
        };
    };
    let project_root = PathBuf::from(project_root);
    let candidate = match crate::runtime_server_admission::discover_workspace_generation_candidate(
        &project_root,
    )
    .await
    {
        Ok(candidate) => candidate,
        Err(message) => {
            return WorkspaceDbIpcResult::Failed {
                code: "runtime-server-generation-candidate-discovery-failed".to_owned(),
                message,
            };
        }
    };
    let ensured = async {
        let receipt = admission
            .ensure(workspace_identity, &project_root, candidate)
            .await?;
        let terminal = if receipt.state
            == crate::runtime_server_admission::WorkspaceGenerationAdmissionState::Building
        {
            admission
                .wait_terminal(workspace_identity, &project_root)
                .await?
        } else {
            receipt
        };
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
        let (commit, reconciled) = match memory_registry
            .published_generation_state(workspace_identity, &project_root)
            .await?
        {
            crate::runtime_server_workspace::PublishedWorkspaceGenerationState::Ready => {
                let generation = memory_registry.lease(workspace_identity, &project_root)?;
                (
                    crate::runtime_server_admission::WorkspaceGenerationCommitReceipt {
                        active_epoch: generation.epoch(),
                        generation_digest: generation.runtime_generation_digest(),
                        source_root_digest: generation
                            .generation()
                            .source_snapshot
                            .root_digest
                            .clone(),
                    },
                    false,
                )
            }
            crate::runtime_server_workspace::PublishedWorkspaceGenerationState::Missing
            | crate::runtime_server_workspace::PublishedWorkspaceGenerationState::RecoveryRequired { .. } => {
                let restored = memory_registry
                    .restore_published_generation(
                        format!("explicit-query-ready-{workspace_identity}"),
                        workspace_identity.to_owned(),
                        &project_root,
                    )
                    .await?;
                (
                    crate::runtime_server_admission::WorkspaceGenerationCommitReceipt::from_recovery(
                        &restored,
                    )?,
                    true,
                )
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
        crate::runtime_server_admission::WorkspaceGenerationReadinessReceipt::new(
            workspace_identity.to_owned(),
            commit,
            reconciled,
        )
    }
    .await;
    match ensured {
        Ok(receipt) => WorkspaceDbIpcResult::RuntimeGenerationReadiness { receipt },
        Err(message) => WorkspaceDbIpcResult::Failed {
            code: "runtime-server-generation-ready-gate-failed".to_owned(),
            message,
        },
    }
}

pub(super) async fn ensure_owner_terminal_ready(
    memory_registry: &RuntimeServerWorkspaceRegistry,
    generation_admission: Option<&WorkspaceGenerationAdmission>,
    workspace_identity: &str,
    project_root: String,
    owner_path: String,
    admitted_content_digest: String,
) -> WorkspaceDbIpcResult {
    let project_root_path = PathBuf::from(&project_root);
    let live_content_digest = match crate::runtime_server_workspace::current_owner_content_digest(
        &project_root_path,
        &owner_path,
    )
    .await
    {
        Ok(digest) => digest,
        Err(message) => {
            return WorkspaceDbIpcResult::Failed {
                code: "runtime-server-owner-freshness-read-failed".to_owned(),
                message,
            };
        }
    };
    if live_content_digest.as_deref() == Some(admitted_content_digest.as_str()) {
        match memory_registry.lease(workspace_identity, &project_root_path) {
            Ok(generation) => {
                let commit = crate::runtime_server_admission::WorkspaceGenerationCommitReceipt {
                    active_epoch: generation.epoch(),
                    generation_digest: generation.runtime_generation_digest(),
                    source_root_digest: generation.generation().source_snapshot.root_digest.clone(),
                };
                return match crate::runtime_server_admission::WorkspaceOwnerGenerationReadinessReceipt::new(
                    workspace_identity.to_owned(),
                    owner_path,
                    live_content_digest,
                    commit,
                    false,
                ) {
                    Ok(receipt) => WorkspaceDbIpcResult::RuntimeOwnerGenerationReadiness { receipt },
                    Err(message) => WorkspaceDbIpcResult::Failed {
                        code: "runtime-server-owner-freshness-receipt-failed".to_owned(),
                        message,
                    },
                };
            }
            Err(_) => {}
        }
    }
    let Some(generation_admission) = generation_admission else {
        return WorkspaceDbIpcResult::Failed {
            code: "runtime-server-owner-projection-builder-unavailable".to_owned(),
            message: "runtime owner freshness requires the ServerProcess owner projection builder"
                .to_owned(),
        };
    };
    let mutation_id = format!(
        "exact-owner-freshness:{}:{}",
        owner_path,
        live_content_digest.as_deref().unwrap_or("missing")
    );
    let publication = if live_content_digest.is_some() {
        let language_id = memory_registry
            .lease(workspace_identity, &project_root_path)
            .ok()
            .and_then(|lease| lease.runtime_owner_snapshot(&owner_path))
            .and_then(|(_, owner)| {
                owner.selectors.into_iter().find_map(|selector| {
                    selector
                        .selector
                        .split_once("://")
                        .map(|(language_id, _)| language_id.to_owned())
                })
            });
        let Some(language_id) = language_id else {
            return WorkspaceDbIpcResult::Failed {
                code: "runtime-server-owner-language-unavailable".to_owned(),
                message: format!(
                    "runtime owner freshness cannot derive parser language: ownerPath={owner_path}"
                ),
            };
        };
        let owner = match generation_admission
            .project_owner(
                workspace_identity.to_owned(),
                project_root_path.clone(),
                owner_path.clone(),
                language_id,
            )
            .await
        {
            Ok(owner) => owner,
            Err(message) => {
                return WorkspaceDbIpcResult::Failed {
                    code: "runtime-server-owner-projection-failed".to_owned(),
                    message,
                };
            }
        };
        memory_registry
            .publish_owner_overlay(
                mutation_id.clone(),
                workspace_identity,
                &project_root_path,
                owner,
            )
            .await
    } else {
        memory_registry
            .tombstone_owner_overlay(
                mutation_id.clone(),
                workspace_identity,
                &project_root_path,
                owner_path.clone(),
            )
            .await
    };
    let published = match publication {
        Ok(receipt) => receipt,
        Err(message) => {
            return WorkspaceDbIpcResult::Failed {
                code: "runtime-server-owner-incremental-publication-failed".to_owned(),
                message,
            };
        }
    };
    let candidate = match crate::runtime_server_admission::discover_workspace_generation_candidate(
        &project_root_path,
    )
    .await
    {
        Ok(candidate) => candidate,
        Err(message) => {
            return WorkspaceDbIpcResult::Failed {
                code: "runtime-server-owner-freshness-candidate-discovery-failed".to_owned(),
                message,
            };
        }
    };
    let admission = admit_mutation(
        memory_registry,
        Some(generation_admission),
        workspace_identity,
        mutation_id,
        project_root.clone(),
        vec![owner_path.clone()],
        candidate,
    )
    .await;
    if !matches!(
        admission,
        WorkspaceDbIpcResult::RuntimeGenerationMutationAdmission { .. }
    ) {
        return admission;
    }
    let commit =
        match crate::runtime_server_admission::WorkspaceGenerationCommitReceipt::from_recovery(
            &published,
        ) {
            Ok(commit) => commit,
            Err(message) => {
                return WorkspaceDbIpcResult::Failed {
                    code: "runtime-server-owner-publication-receipt-failed".to_owned(),
                    message,
                };
            }
        };
    match crate::runtime_server_admission::WorkspaceOwnerGenerationReadinessReceipt::new(
        workspace_identity.to_owned(),
        owner_path,
        live_content_digest,
        commit,
        true,
    ) {
        Ok(receipt) => WorkspaceDbIpcResult::RuntimeOwnerGenerationReadiness { receipt },
        Err(message) => WorkspaceDbIpcResult::Failed {
            code: "runtime-server-owner-freshness-receipt-failed".to_owned(),
            message,
        },
    }
}
