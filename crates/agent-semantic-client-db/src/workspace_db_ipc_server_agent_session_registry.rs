//! Typed agent-session registry operations owned by the Runtime Server.

pub(super) async fn run_operation(
    registry: std::sync::Arc<crate::AgentSessionRegistry>,
    project_root: std::path::PathBuf,
    operation: crate::workspace_db_ipc::AgentSessionRegistryIpcOperation,
) -> Result<crate::workspace_db_ipc::AgentSessionRegistryIpcResult, String> {
    let state_root = crate::AgentSessionRegistry::state_root_for_project(&project_root)?;
    let requested_db_path = crate::AgentSessionRegistry::db_path_for_state_root(state_root);
    if requested_db_path != registry.db_path() {
        return Err(format!(
            "agent-session registry owner state-root mismatch: owner={} requested={}",
            registry.db_path().display(),
            requested_db_path.display()
        ));
    }
    tokio::task::spawn_blocking(move || {
        use crate::workspace_db_ipc::{
            AgentSessionRegistryIpcOperation as Operation,
            AgentSessionRegistryIpcResult as IpcResult,
        };

        match operation {
            Operation::Register { request } => {
                let model_source = request
                    .model_observation
                    .as_ref()
                    .map(|observation| match observation.source.as_str() {
                        "codex.subagent-start" => Ok(
                            crate::agent_session_registry::AgentSessionModelObservationSource::CodexSubagentStart,
                        ),
                        "codex.rollout" => Ok(
                            crate::agent_session_registry::AgentSessionModelObservationSource::CodexRollout,
                        ),
                        source => Err(format!(
                            "unsupported agent-session model observation source: {source}"
                        )),
                    })
                    .transpose()?;
                let model_observation = match (request.model_observation.as_ref(), model_source) {
                    (Some(observation), Some(source)) => Some(
                        crate::agent_session_registry::AgentSessionModelObservationRef {
                            model: observation.model.as_str(),
                            source,
                            observed_at: observation.observed_at,
                            evidence_ref: observation.evidence_ref.as_deref(),
                        },
                    ),
                    (None, None) => None,
                    _ => {
                        return Err(
                            "agent-session model observation/source presence mismatch".to_owned()
                        );
                    }
                };
                let session = registry.register_session(crate::AgentSessionRegisterRequest {
                    project_id: request.project_id.into(),
                    root_session_id: request.root_session_id.into(),
                    session_id: request.session_id.into(),
                    message_target_id: request.message_target_id.map(Into::into),
                    parent_session_id: request.parent_session_id.map(Into::into),
                    name: request.name.into(),
                    role: request.role.into(),
                    model_observation,
                    status: request.status.into(),
                    expires_at: request.expires_at,
                    metadata_json: request.metadata_json.into(),
                    now: request.now,
                })?;
                Ok(IpcResult::Registered { session })
            }
            Operation::Query {
                project_id,
                root_session_id,
                name,
            } => Ok(IpcResult::Sessions {
                sessions: registry.query_sessions(
                    project_id,
                    root_session_id.map(Into::into),
                    name.map(Into::into),
                )?,
            }),
            Operation::SessionById {
                project_id,
                session_id,
            } => Ok(IpcResult::Session {
                session: registry.session_by_id(project_id, session_id)?,
            }),
            Operation::SessionByName {
                project_id,
                root_session_id,
                name,
            } => Ok(IpcResult::Session {
                session: registry.session_by_name(project_id, root_session_id, name)?,
            }),
            Operation::UpdateStatus {
                project_id,
                session_id,
                status,
                now,
            } => Ok(IpcResult::Changed {
                changed: registry.update_session_status(project_id, session_id, status, now)?,
            }),
            Operation::SetArchivedStatus {
                project_id,
                session_id,
                archived,
                now,
            } => Ok(IpcResult::Changed {
                changed: if archived {
                    registry.archive_session(project_id, session_id, now)?
                } else {
                    registry.unarchive_session(project_id, session_id, now)?
                },
            }),
            Operation::SessionIsRetired {
                project_id,
                session_id,
            } => Ok(IpcResult::Changed {
                changed: registry.session_is_retired(project_id, session_id)?,
            }),
            Operation::RefreshExpired => {
                registry.refresh_expired_sessions()?;
                Ok(IpcResult::Refreshed)
            }
            Operation::SessionByIdAnyProject { session_id } => Ok(IpcResult::Session {
                session: registry.session_by_id_any_project(session_id)?,
            }),
            Operation::ProjectIdForRootSessionId { root_session_id } => Ok(IpcResult::ProjectId {
                project_id: registry.project_id_for_root_session_id(root_session_id)?,
            }),
            Operation::ClaimDispatch {
                project_id,
                root_session_id,
                name,
                dispatch_identity,
                command_digest,
                delivery_target_override,
                now,
            } => Ok(IpcResult::DispatchClaimed {
                result: registry.claim_dispatch(
                    crate::agent_session_registry::AgentSessionDispatchClaimRequest {
                        project_id: &project_id,
                        root_session_id: &root_session_id,
                        name: &name,
                        dispatch_identity: &dispatch_identity,
                        command_digest: &command_digest,
                        delivery_target_override: delivery_target_override.as_deref(),
                        now,
                    },
                )?,
            }),
        }
    })
    .await
    .map_err(|error| format!("agent-session registry owner task failed: {error}"))?
}
