//! Runtime Server-owned cache-control evaluation for workspace generations.

use std::path::Path;

use crate::runtime_server_admission::{
    WorkspaceGenerationAdmission, WorkspaceGenerationAdmissionState,
    discover_workspace_generation_candidate,
};
use crate::runtime_server_workspace::RuntimeServerWorkspaceRegistry;
use crate::workspace_db_ipc::{
    RuntimeCacheControlReceipt, RuntimeCacheControlRequest, RuntimeCacheGenerationState,
    WorkspaceDbIpcResult,
};

fn receipt(
    action: &str,
    generation_state: RuntimeCacheGenerationState,
    generation_digest: Option<String>,
    mutation_id: Option<String>,
) -> WorkspaceDbIpcResult {
    WorkspaceDbIpcResult::CacheControl {
        receipt: RuntimeCacheControlReceipt {
            action: action.to_owned(),
            authority: "runtime-server".to_owned(),
            generation_state,
            generation_digest,
            database_opens_by_client: 0,
            writer_queue_owner: "runtime-server".to_owned(),
            mutation_id,
            failure: None,
        },
    }
}

fn status_receipt(
    generation_state: RuntimeCacheGenerationState,
    generation_digest: Option<String>,
    failure: Option<String>,
) -> WorkspaceDbIpcResult {
    WorkspaceDbIpcResult::CacheControl {
        receipt: RuntimeCacheControlReceipt {
            action: "status".to_owned(),
            authority: "runtime-server".to_owned(),
            generation_state,
            generation_digest,
            database_opens_by_client: 0,
            writer_queue_owner: "runtime-server".to_owned(),
            mutation_id: None,
            failure,
        },
    }
}

async fn admit_generation(
    admission: &WorkspaceGenerationAdmission,
    workspace_identity: &str,
    project_root: &Path,
) -> Result<(RuntimeCacheGenerationState, Option<String>), String> {
    let candidate = discover_workspace_generation_candidate(project_root).await?;
    let admitted = admission
        .admit(
            workspace_identity.to_owned(),
            project_root.to_path_buf(),
            candidate,
        )
        .await?;
    let state = if admitted.state == WorkspaceGenerationAdmissionState::Ready {
        RuntimeCacheGenerationState::Ready
    } else {
        RuntimeCacheGenerationState::Rebuilding
    };
    Ok((
        state,
        admitted.commit.map(|commit| commit.generation_digest),
    ))
}

async fn rebuild_generation(
    admission: &WorkspaceGenerationAdmission,
    workspace_identity: &str,
    project_root: &Path,
    mutation_id: &str,
) -> Result<(RuntimeCacheGenerationState, Option<String>), String> {
    let candidate = discover_workspace_generation_candidate(project_root).await?;
    let admitted = admission
        .admit_cache_rebuild(
            mutation_id.to_owned(),
            workspace_identity.to_owned(),
            project_root.to_path_buf(),
            candidate,
        )
        .await?;
    let state = if admitted.state == WorkspaceGenerationAdmissionState::Ready {
        RuntimeCacheGenerationState::Ready
    } else {
        RuntimeCacheGenerationState::Rebuilding
    };
    Ok((
        state,
        admitted.commit.map(|commit| commit.generation_digest),
    ))
}

fn admission_unavailable() -> WorkspaceDbIpcResult {
    WorkspaceDbIpcResult::Failed {
        code: "runtime-server-generation-admission-unavailable".to_owned(),
        message: "Runtime Server has no canonical generation admission owner".to_owned(),
    }
}

/// Evaluate one v1 cache-control request without granting the client database authority.
pub(super) async fn evaluate(
    memory_registry: &RuntimeServerWorkspaceRegistry,
    generation_admission: Option<&WorkspaceGenerationAdmission>,
    workspace_identity: &str,
    request: RuntimeCacheControlRequest,
) -> WorkspaceDbIpcResult {
    match request {
        RuntimeCacheControlRequest::Status { project_root } => {
            match memory_registry.lease(workspace_identity, Path::new(&project_root)) {
                Ok(generation) => receipt(
                    "status",
                    RuntimeCacheGenerationState::Ready,
                    Some(generation.runtime_generation_digest()),
                    None,
                ),
                Err(_) => match generation_admission {
                    Some(admission) => match admission
                        .status(workspace_identity, Path::new(&project_root))
                        .await
                    {
                        Some(admitted) => status_receipt(
                            match admitted.state {
                                WorkspaceGenerationAdmissionState::Building => {
                                    RuntimeCacheGenerationState::Rebuilding
                                }
                                WorkspaceGenerationAdmissionState::Ready => {
                                    RuntimeCacheGenerationState::Ready
                                }
                                WorkspaceGenerationAdmissionState::Failed
                                | WorkspaceGenerationAdmissionState::Cancelled => {
                                    RuntimeCacheGenerationState::Stale
                                }
                            },
                            admitted.commit.map(|commit| commit.generation_digest),
                            admitted.error,
                        ),
                        None => receipt("status", RuntimeCacheGenerationState::Missing, None, None),
                    },
                    None => receipt("status", RuntimeCacheGenerationState::Missing, None, None),
                },
            }
        }
        RuntimeCacheControlRequest::RefreshSourceIndex {
            project_root,
            expected_generation,
        } => {
            let current_digest = memory_registry
                .lease(workspace_identity, Path::new(&project_root))
                .ok()
                .map(|generation| generation.runtime_generation_digest());
            if current_digest.is_some()
                && expected_generation
                    .as_ref()
                    .is_none_or(|expected| current_digest.as_ref() == Some(expected))
            {
                return receipt(
                    "refresh-source-index",
                    RuntimeCacheGenerationState::Ready,
                    current_digest,
                    None,
                );
            }
            let Some(admission) = generation_admission else {
                return admission_unavailable();
            };
            match admit_generation(admission, workspace_identity, Path::new(&project_root)).await {
                Ok((state, digest)) => receipt("refresh-source-index", state, digest, None),
                Err(message) => WorkspaceDbIpcResult::Failed {
                    code: "runtime-server-cache-refresh-failed".to_owned(),
                    message,
                },
            }
        }
        RuntimeCacheControlRequest::RebuildSourceIndex {
            project_root,
            mutation_id,
        } => {
            let Some(admission) = generation_admission else {
                return admission_unavailable();
            };
            match rebuild_generation(
                admission,
                workspace_identity,
                Path::new(&project_root),
                &mutation_id,
            )
            .await
            {
                Ok((state, digest)) => {
                    receipt("rebuild-source-index", state, digest, Some(mutation_id))
                }
                Err(message) => WorkspaceDbIpcResult::Failed {
                    code: "runtime-server-cache-rebuild-failed".to_owned(),
                    message,
                },
            }
        }
        RuntimeCacheControlRequest::Invalidate {
            project_root,
            mutation_id,
            ..
        } => {
            let Some(admission) = generation_admission else {
                return admission_unavailable();
            };
            match rebuild_generation(
                admission,
                workspace_identity,
                Path::new(&project_root),
                &mutation_id,
            )
            .await
            {
                Ok((state, digest)) => receipt("invalidate", state, digest, Some(mutation_id)),
                Err(message) => WorkspaceDbIpcResult::Failed {
                    code: "runtime-server-cache-invalidate-failed".to_owned(),
                    message,
                },
            }
        }
    }
}
