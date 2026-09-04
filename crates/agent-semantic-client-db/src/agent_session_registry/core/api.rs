//! Synchronous public registry operations over the Turso-owned core.

use super::AgentSessionRegistry;
use super::storage::block_on_agent_session_registry_async;
use super::storage::turso_delete_session;
use super::storage::turso_query_all_sessions;
use super::storage::turso_query_sessions;
use super::storage::turso_record_tool_event;
use super::storage::turso_refresh_expired_sessions;
use super::storage::turso_register_session;
use super::storage::turso_session_by_id;
use super::storage::turso_session_by_id_any_project;
use super::storage::turso_session_by_name;
use super::storage::turso_session_for_root_session_id_any_project;
use super::storage::turso_session_is_retired;
use super::storage::turso_set_archived_status;
use super::storage::turso_update_session_status;
use crate::agent_session_registry::types::AGENT_SESSION_STATUS_ACTIVE;
use crate::agent_session_registry::types::AGENT_SESSION_STATUS_ARCHIVED;
use crate::agent_session_registry::types::AGENT_SESSION_STATUS_INVALID;
use crate::agent_session_registry::types::AgentSessionDispatchClaimRequest;
use crate::agent_session_registry::types::AgentSessionDispatchClaimResult;
use crate::agent_session_registry::types::AgentSessionDispatchCompleteRequest;
use crate::agent_session_registry::types::AgentSessionDispatchLeaseRecord;
use crate::agent_session_registry::types::AgentSessionId;
use crate::agent_session_registry::types::AgentSessionLookupRequest;
use crate::agent_session_registry::types::AgentSessionProjectId;
use crate::agent_session_registry::types::AgentSessionRecord;
use crate::agent_session_registry::types::AgentSessionRegisterRequest;
use crate::agent_session_registry::types::AgentSessionResidentName;
use crate::agent_session_registry::types::AgentSessionRootSessionId;
use crate::agent_session_registry::types::AgentSessionStatus;
use crate::agent_session_registry::types::AgentSessionToolEventRequest;
use crate::agent_session_registry::types::agent_session_unix_timestamp;

impl AgentSessionRegistry {
    pub async fn record_host_execution_observation_async(
        &self,
        observation: crate::workspace_db_ipc::AgentHostExecutionObservationIpc,
    ) -> Result<crate::workspace_db_ipc::AgentSessionRegistryIpcResult, String> {
        self.runtime_operation_async(
            crate::workspace_db_ipc::AgentSessionRegistryIpcOperation::RecordHostExecutionObservation {
                observation,
            },
        )
        .await
    }

    pub(crate) async fn query_all_sessions_local(&self) -> Result<Vec<AgentSessionRecord>, String> {
        turso_query_all_sessions(&self.db_path).await
    }

    /// Register a child directly inside the sole Runtime Server DB owner.
    ///
    /// This is the Multi-Agent v2 mutation boundary. It deliberately avoids
    /// the retired WorkspaceDb IPC proxy and every synchronous `block_on` bridge.
    pub async fn register_session_from_runtime_owner(
        &self,
        request: AgentSessionRegisterRequest<'_>,
    ) -> Result<AgentSessionRecord, String> {
        if !Self::is_runtime_server_owner_process() {
            return Err("Multi-Agent registration requires the Runtime Server DB owner".to_owned());
        }
        if turso_session_is_retired(
            &self.db_path,
            request.project_id.as_str(),
            request.session_id.as_str(),
        )
        .await?
        {
            return Err(format!(
                "retired physical session generation cannot be registered again: projectId={} sessionId={}",
                request.project_id, request.session_id
            ));
        }
        turso_register_session(&self.db_path, request).await
    }

    /// Read the current durable child set inside the Runtime Server DB owner.
    pub async fn query_all_sessions_from_runtime_owner(
        &self,
    ) -> Result<Vec<AgentSessionRecord>, String> {
        if !Self::is_runtime_server_owner_process() {
            return Err("Multi-Agent projection requires the Runtime Server DB owner".to_owned());
        }
        turso_query_all_sessions(&self.db_path).await
    }

    /// Resolve the complete Hook-selected session pane state in one Runtime Server call.
    pub async fn resolve_session_control_plane(
        &self,
        observed_session_id: Option<&str>,
        observed_root_session_id: Option<&str>,
        name: &str,
    ) -> Result<crate::runtime_server_control::AgentSessionControlPlaneState, String> {
        let project_root = self.runtime_project_root.as_deref().ok_or_else(|| {
            "session control-plane resolution requires the Runtime Server owner".to_owned()
        })?;
        Self::resolve_project_session_control_plane(
            project_root,
            observed_session_id,
            observed_root_session_id,
            name,
        )
        .await
    }

    /// Resolve the Hook-selected pane from the Runtime Server status-memory projection.
    ///
    /// This read-only path does not require workspace admission, a Runtime IPC proxy, or a
    /// direct registry/database open. A root task with no projected child is therefore a valid
    /// `registration-required` state.
    pub async fn resolve_project_session_control_plane(
        project_root: &std::path::Path,
        observed_session_id: Option<&str>,
        observed_root_session_id: Option<&str>,
        name: &str,
    ) -> Result<crate::runtime_server_control::AgentSessionControlPlaneState, String> {
        let state = agent_semantic_client_core::state_core::ResolvedState::resolve(project_root)?;
        let endpoint = crate::read_runtime_server_endpoint(&state.state_home)
            .await?
            .ok_or_else(|| "Runtime Server endpoint is unavailable".to_owned())?;
        let workspace_id = Self::workspace_id(project_root)?;
        let sessions = crate::read_runtime_server_agent_sessions(&endpoint).await?;
        crate::resolve_runtime_server_agent_session_status(
            &sessions,
            &workspace_id,
            observed_session_id,
            observed_root_session_id,
            name,
        )
    }

    pub async fn register_session(
        &self,
        request: AgentSessionRegisterRequest<'_>,
    ) -> Result<AgentSessionRecord, String> {
        if self
            .session_is_retired(request.project_id.as_str(), request.session_id.as_str())
            .await?
        {
            return Err(format!(
                "retired physical session generation cannot be registered again: projectId={} sessionId={}",
                request.project_id, request.session_id
            ));
        }
        if let Some(result) = self
            .runtime_operation(
                crate::workspace_db_ipc::AgentSessionRegistryIpcOperation::Register {
                    request: crate::workspace_db_ipc::AgentSessionRegisterIpcRequest {
                        project_id: request.project_id.clone(),
                        root_session_id: request.root_session_id.clone(),
                        session_id: request.session_id.clone(),
                        message_target_id: request.message_target_id.clone(),
                        parent_session_id: request.parent_session_id.clone(),
                        name: request.name.clone(),
                        role: request.role.clone(),
                        model_observation: request.model_observation.map(|observation| {
                            crate::workspace_db_ipc::AgentSessionModelObservationIpc {
                                model: observation.model.into(),
                                source: observation.source.as_str().into(),
                                observed_at: observation.observed_at,
                                evidence_ref: observation.evidence_ref.map(Into::into),
                            }
                        }),
                        status: request.status.clone(),
                        expires_at: request.expires_at,
                        metadata_json: request.metadata_json.clone(),
                        now: request.now,
                    },
                },
            )
            .await?
        {
            return match result {
                crate::workspace_db_ipc::AgentSessionRegistryIpcResult::Registered { session } => {
                    Ok(session)
                }
                _ => Err(
                    "Runtime Server returned an unexpected session registration result".to_owned(),
                ),
            };
        }
        block_on_agent_session_registry_async(turso_register_session(&self.db_path, request))
    }

    /// Publish one Host-native child lifecycle event through the Runtime Server owner.
    pub async fn record_host_lifecycle_event(
        &self,
        event: crate::workspace_db_ipc::AgentHostLifecycleEventIpc,
    ) -> Result<crate::workspace_db_ipc::AgentSessionRegistryIpcResult, String> {
        self.runtime_operation(
            crate::workspace_db_ipc::AgentSessionRegistryIpcOperation::RecordHostLifecycleEvent {
                event,
            },
        )
        .await?
        .ok_or_else(|| "Host lifecycle events require the Runtime Server registry proxy".to_owned())
    }

    pub async fn record_host_lifecycle_event_async(
        &self,
        event: crate::workspace_db_ipc::AgentHostLifecycleEventIpc,
    ) -> Result<crate::workspace_db_ipc::AgentSessionRegistryIpcResult, String> {
        self.runtime_operation_async(
            crate::workspace_db_ipc::AgentSessionRegistryIpcOperation::RecordHostLifecycleEvent {
                event,
            },
        )
        .await
    }

    pub async fn record_host_non_match(
        &self,
        observation: crate::workspace_db_ipc::AgentHostNonMatchIpc,
    ) -> Result<crate::workspace_db_ipc::AgentSessionRegistryIpcResult, String> {
        self.runtime_operation(
            crate::workspace_db_ipc::AgentSessionRegistryIpcOperation::RecordHostNonMatch {
                observation,
            },
        )
        .await?
        .ok_or_else(|| "Host match decisions require the Runtime Server registry proxy".to_owned())
    }

    pub async fn register_control_plane_agent(
        &self,
        registration: crate::SessionControlPlaneAgentRegistration,
    ) -> Result<(), String> {
        match self
            .runtime_operation_async(
                crate::workspace_db_ipc::AgentSessionRegistryIpcOperation::RegisterControlPlaneAgent {
                    registration,
                },
            )
            .await?
        {
            crate::workspace_db_ipc::AgentSessionRegistryIpcResult::ControlPlaneAgentRegistered => {
                Ok(())
            }
            result => Err(format!(
                "Runtime Server returned unexpected control-plane registration result: {result:?}"
            )),
        }
    }

    pub async fn admit_control_plane_delegation(
        &self,
        proposal: crate::SessionControlPlaneDelegationProposal,
    ) -> Result<crate::SessionControlPlaneTransactionReceipt, String> {
        match self
            .runtime_operation_async(
                crate::workspace_db_ipc::AgentSessionRegistryIpcOperation::AdmitControlPlaneDelegation {
                    proposal,
                },
            )
            .await?
        {
            crate::workspace_db_ipc::AgentSessionRegistryIpcResult::ControlPlaneDelegationAdmitted {
                receipt,
            } => Ok(receipt),
            result => Err(format!(
                "Runtime Server returned unexpected control-plane admission result: {result:?}"
            )),
        }
    }

    pub async fn read_control_plane_snapshot(
        &self,
        project_id: String,
        root_session_id: String,
    ) -> Result<crate::SessionControlPlaneSnapshot, String> {
        match self
            .runtime_operation_async(
                crate::workspace_db_ipc::AgentSessionRegistryIpcOperation::ReadControlPlaneSnapshot {
                    project_id: project_id.into(),
                    root_session_id: root_session_id.into(),
                },
            )
            .await?
        {
            crate::workspace_db_ipc::AgentSessionRegistryIpcResult::ControlPlaneSnapshot {
                snapshot,
            } => Ok(snapshot),
            result => Err(format!(
                "Runtime Server returned unexpected control-plane snapshot result: {result:?}"
            )),
        }
    }

    /// Return registered sessions for one project, optionally narrowed by root session and name.
    pub async fn query_sessions(
        &self,
        project_id: impl Into<AgentSessionProjectId>,
        root_session_id: Option<AgentSessionRootSessionId>,
        name: Option<AgentSessionResidentName>,
    ) -> Result<Vec<AgentSessionRecord>, String> {
        let project_id = project_id.into();
        if let Some(result) = self
            .runtime_operation(
                crate::workspace_db_ipc::AgentSessionRegistryIpcOperation::Query {
                    project_id: project_id.clone(),
                    root_session_id: root_session_id.clone(),
                    name: name.clone(),
                },
            )
            .await?
        {
            return match result {
                crate::workspace_db_ipc::AgentSessionRegistryIpcResult::Sessions { sessions } => {
                    Ok(sessions)
                }
                _ => Err("Runtime Server returned an unexpected session query result".to_owned()),
            };
        }
        block_on_agent_session_registry_async(turso_query_sessions(
            &self.db_path,
            project_id.as_str(),
            root_session_id
                .as_ref()
                .map(AgentSessionRootSessionId::as_str),
            name.as_ref().map(AgentSessionResidentName::as_str),
        ))
    }

    /// Query the registry through its local async owner path.
    ///
    /// Runtime Server handlers must use this entry point instead of
    /// `query_sessions`: the public synchronous adapter may proxy back through
    /// the Runtime Server and would otherwise recursively enter the same IPC
    /// owner while it is materializing a control-plane snapshot.
    /// Return one registered session by its concrete session id.
    pub async fn session_by_id(
        &self,
        project_id: impl Into<AgentSessionProjectId>,
        session_id: impl Into<AgentSessionId>,
    ) -> Result<Option<AgentSessionRecord>, String> {
        let project_id = project_id.into();
        let session_id = session_id.into();
        if let Some(result) = self
            .runtime_operation(
                crate::workspace_db_ipc::AgentSessionRegistryIpcOperation::SessionById {
                    project_id: project_id.clone(),
                    session_id: session_id.clone(),
                },
            )
            .await?
        {
            return match result {
                crate::workspace_db_ipc::AgentSessionRegistryIpcResult::Session { session } => {
                    Ok(session)
                }
                _ => Err("Runtime Server returned an unexpected session lookup result".to_owned()),
            };
        }
        block_on_agent_session_registry_async(turso_session_by_id(
            &self.db_path,
            project_id.as_str(),
            session_id.as_str(),
        ))
    }

    /// Return one registered session by its stable root/name route.
    pub async fn session_by_name(
        &self,
        project_id: impl Into<AgentSessionProjectId>,
        root_session_id: impl Into<AgentSessionRootSessionId>,
        name: impl Into<AgentSessionResidentName>,
    ) -> Result<Option<AgentSessionRecord>, String> {
        let project_id = project_id.into();
        let root_session_id = root_session_id.into();
        let name = name.into();
        if let Some(result) = self
            .runtime_operation(
                crate::workspace_db_ipc::AgentSessionRegistryIpcOperation::SessionByName {
                    project_id: project_id.clone(),
                    root_session_id: root_session_id.clone(),
                    name: name.clone(),
                },
            )
            .await?
        {
            return match result {
                crate::workspace_db_ipc::AgentSessionRegistryIpcResult::Session { session } => {
                    Ok(session)
                }
                _ => Err(
                    "Runtime Server returned an unexpected named session lookup result".to_owned(),
                ),
            };
        }
        block_on_agent_session_registry_async(turso_session_by_name(
            &self.db_path,
            project_id.as_str(),
            root_session_id.as_str(),
            name.as_str(),
        ))
    }

    /// Record the latest tool event for one registered session.
    pub fn record_tool_event(&self, request: AgentSessionToolEventRequest) -> Result<bool, String> {
        block_on_agent_session_registry_async(turso_record_tool_event(&self.db_path, request))
    }

    /// Claim, poll, or rebind one exact resident-child dispatch identity.
    pub async fn claim_dispatch(
        &self,
        request: AgentSessionDispatchClaimRequest<'_>,
    ) -> Result<AgentSessionDispatchClaimResult, String> {
        if let Some(result) = self
            .runtime_operation(
                crate::workspace_db_ipc::AgentSessionRegistryIpcOperation::ClaimDispatch {
                    project_id: request.project_id.into(),
                    root_session_id: request.root_session_id.into(),
                    child_session_id: request.child_session_id.into(),
                    name: request.name.into(),
                    dispatch_identity: request.dispatch_identity.into(),
                    command_digest: request.command_digest.into(),
                    delivery_target_override: request.delivery_target_override.map(Into::into),
                    now: request.now,
                },
            )
            .await?
        {
            return match result {
                crate::workspace_db_ipc::AgentSessionRegistryIpcResult::DispatchClaimed {
                    result,
                } => Ok(result),
                _ => Err("Runtime Server returned an unexpected dispatch claim result".to_owned()),
            };
        }
        block_on_agent_session_registry_async(
            crate::agent_session_registry::dispatch::turso_claim_dispatch(&self.db_path, request),
        )
    }

    /// Record the terminal receipt for one exact resident-child dispatch identity.
    pub async fn complete_dispatch(
        &self,
        request: AgentSessionDispatchCompleteRequest<'_>,
    ) -> Result<AgentSessionDispatchLeaseRecord, String> {
        if let Some(result) = self
            .runtime_operation(
                crate::workspace_db_ipc::AgentSessionRegistryIpcOperation::CompleteDispatch {
                    project_id: request.project_id.into(),
                    root_session_id: request.root_session_id.into(),
                    name: request.name.into(),
                    dispatch_identity: request.dispatch_identity.into(),
                    command_digest: request.command_digest.into(),
                    evidence_ref: request.evidence_ref.into(),
                    now: request.now,
                },
            )
            .await?
        {
            return match result {
                crate::workspace_db_ipc::AgentSessionRegistryIpcResult::DispatchCompleted {
                    lease,
                } => Ok(lease),
                _ => Err(
                    "Runtime Server returned an unexpected dispatch completion result".to_owned(),
                ),
            };
        }
        block_on_agent_session_registry_async(
            crate::agent_session_registry::dispatch::turso_complete_dispatch(
                &self.db_path,
                request,
            ),
        )
    }

    /// Mark an in-flight resident-child dispatch as awaiting a verified rebind.
    pub fn mark_dispatch_orphaned(
        &self,
        request: crate::agent_session_registry::types::AgentSessionDispatchMarkOrphanedRequest<'_>,
    ) -> Result<AgentSessionDispatchLeaseRecord, String> {
        block_on_agent_session_registry_async(
            crate::agent_session_registry::dispatch::turso_mark_dispatch_orphaned(
                &self.db_path,
                request,
            ),
        )
    }

    /// Generic lookup used by registry CLI commands.
    pub async fn lookup_session(
        &self,
        request: AgentSessionLookupRequest,
    ) -> Result<Option<AgentSessionRecord>, String> {
        if let Some(session_id) = request.session_id.as_ref() {
            return self
                .session_by_id(request.project_id.clone(), session_id.clone())
                .await;
        }
        if let (Some(root_session_id), Some(name)) =
            (request.root_session_id.as_ref(), request.name.as_ref())
        {
            return self
                .session_by_name(
                    request.project_id.clone(),
                    root_session_id.clone(),
                    name.clone(),
                )
                .await;
        }
        let sessions = self
            .query_sessions(request.project_id, request.root_session_id, request.name)
            .await?;
        Ok(sessions.into_iter().next())
    }

    /// Update one session row to the supplied routing status.
    pub async fn update_session_status(
        &self,
        project_id: impl Into<AgentSessionProjectId>,
        session_id: impl Into<AgentSessionId>,
        status: impl Into<AgentSessionStatus>,
        now: i64,
    ) -> Result<bool, String> {
        let project_id = project_id.into();
        let session_id = session_id.into();
        let status = status.into();
        if let Some(result) = self
            .runtime_operation(
                crate::workspace_db_ipc::AgentSessionRegistryIpcOperation::UpdateStatus {
                    project_id: project_id.clone(),
                    session_id: session_id.clone(),
                    status: status.clone(),
                    now,
                },
            )
            .await?
        {
            return match result {
                crate::workspace_db_ipc::AgentSessionRegistryIpcResult::Changed { changed } => {
                    Ok(changed)
                }
                _ => Err("Runtime Server returned an unexpected session status result".to_owned()),
            };
        }
        block_on_agent_session_registry_async(turso_update_session_status(
            &self.db_path,
            project_id.as_str(),
            session_id.as_str(),
            status.as_str(),
            now,
        ))
    }

    /// Mark one session row invalid.
    pub async fn mark_session_invalid(
        &self,
        project_id: impl Into<AgentSessionProjectId>,
        session_id: impl Into<AgentSessionId>,
        now: i64,
    ) -> Result<bool, String> {
        let project_id = project_id.into();
        let session_id = session_id.into();
        self.update_session_status(project_id, session_id, AGENT_SESSION_STATUS_INVALID, now)
            .await
    }

    /// Archive one session row.
    pub async fn archive_session(
        &self,
        project_id: impl Into<AgentSessionProjectId>,
        session_id: impl Into<AgentSessionId>,
        now: i64,
    ) -> Result<bool, String> {
        let project_id = project_id.into();
        let session_id = session_id.into();
        if let Some(result) = self
            .runtime_operation(
                crate::workspace_db_ipc::AgentSessionRegistryIpcOperation::SetArchivedStatus {
                    project_id: project_id.clone(),
                    session_id: session_id.clone(),
                    archived: true,
                    now,
                },
            )
            .await?
        {
            return match result {
                crate::workspace_db_ipc::AgentSessionRegistryIpcResult::Changed { changed } => {
                    Ok(changed)
                }
                _ => Err("Runtime Server returned an unexpected session archive result".to_owned()),
            };
        }
        block_on_agent_session_registry_async(turso_set_archived_status(
            &self.db_path,
            project_id.as_str(),
            session_id.as_str(),
            AGENT_SESSION_STATUS_ARCHIVED,
            Some(now),
            now,
        ))
    }

    /// Unarchive one session row.
    pub async fn unarchive_session(
        &self,
        project_id: impl Into<AgentSessionProjectId>,
        session_id: impl Into<AgentSessionId>,
        now: i64,
    ) -> Result<bool, String> {
        let project_id = project_id.into();
        let session_id = session_id.into();
        if let Some(result) = self
            .runtime_operation(
                crate::workspace_db_ipc::AgentSessionRegistryIpcOperation::SetArchivedStatus {
                    project_id: project_id.clone(),
                    session_id: session_id.clone(),
                    archived: false,
                    now,
                },
            )
            .await?
        {
            return match result {
                crate::workspace_db_ipc::AgentSessionRegistryIpcResult::Changed { changed } => {
                    Ok(changed)
                }
                _ => {
                    Err("Runtime Server returned an unexpected session unarchive result".to_owned())
                }
            };
        }
        block_on_agent_session_registry_async(turso_set_archived_status(
            &self.db_path,
            project_id.as_str(),
            session_id.as_str(),
            AGENT_SESSION_STATUS_ACTIVE,
            None,
            now,
        ))
    }

    /// Delete one session row.
    pub fn delete_session(
        &self,
        project_id: impl Into<AgentSessionProjectId>,
        session_id: impl Into<AgentSessionId>,
    ) -> Result<bool, String> {
        let project_id = project_id.into();
        let session_id = session_id.into();
        block_on_agent_session_registry_async(turso_delete_session(
            &self.db_path,
            project_id.as_str(),
            session_id.as_str(),
        ))
    }

    /// Return whether an exact physical session generation has been retired.
    pub async fn session_is_retired(
        &self,
        project_id: impl Into<AgentSessionProjectId>,
        session_id: impl Into<AgentSessionId>,
    ) -> Result<bool, String> {
        let project_id = project_id.into();
        let session_id = session_id.into();
        if let Some(result) = self
            .runtime_operation(
                crate::workspace_db_ipc::AgentSessionRegistryIpcOperation::SessionIsRetired {
                    project_id: project_id.clone(),
                    session_id: session_id.clone(),
                },
            )
            .await?
        {
            return match result {
                crate::workspace_db_ipc::AgentSessionRegistryIpcResult::Changed { changed } => {
                    Ok(changed)
                }
                _ => Err("Runtime Server returned an unexpected retired-session result".to_owned()),
            };
        }
        block_on_agent_session_registry_async(super::storage::turso_session_is_retired(
            self.db_path(),
            project_id.as_str(),
            session_id.as_str(),
        ))
    }

    /// Refresh expired routable sessions in this registry DB.
    pub async fn refresh_expired_sessions(&self) -> Result<(), String> {
        if let Some(result) = self
            .runtime_operation(
                crate::workspace_db_ipc::AgentSessionRegistryIpcOperation::RefreshExpired,
            )
            .await?
        {
            return match result {
                crate::workspace_db_ipc::AgentSessionRegistryIpcResult::Refreshed => Ok(()),
                _ => Err("Runtime Server returned an unexpected session refresh result".to_owned()),
            };
        }
        let now = agent_session_unix_timestamp()?;
        block_on_agent_session_registry_async(turso_refresh_expired_sessions(&self.db_path, now))
    }

    /// Return one registered session by its concrete session id across all projects.
    pub async fn session_by_id_any_project(
        &self,
        session_id: impl Into<AgentSessionId>,
    ) -> Result<Option<AgentSessionRecord>, String> {
        let session_id = session_id.into();
        if let Some(result) = self
            .runtime_operation(
                crate::workspace_db_ipc::AgentSessionRegistryIpcOperation::SessionByIdAnyProject {
                    session_id: session_id.clone(),
                },
            )
            .await?
        {
            return match result {
                crate::workspace_db_ipc::AgentSessionRegistryIpcResult::Session { session } => {
                    Ok(session)
                }
                _ => Err(
                    "Runtime Server returned an unexpected global session lookup result".to_owned(),
                ),
            };
        }
        block_on_agent_session_registry_async(turso_session_by_id_any_project(
            &self.db_path,
            session_id.as_str(),
        ))
    }

    /// Return the project id for the most recent session registered under one root.
    pub async fn project_id_for_root_session_id(
        &self,
        root_session_id: impl Into<AgentSessionRootSessionId>,
    ) -> Result<Option<String>, String> {
        let root_session_id = root_session_id.into();
        if let Some(result) = self.runtime_operation(
            crate::workspace_db_ipc::AgentSessionRegistryIpcOperation::ProjectIdForRootSessionId {
                root_session_id: root_session_id.clone(),
            },
        ).await? {
            return match result {
                crate::workspace_db_ipc::AgentSessionRegistryIpcResult::ProjectId {
                    project_id,
                } => Ok(project_id.map(|project_id| project_id.into_string())),
                _ => Err(
                    "Runtime Server returned an unexpected root-session project result".to_owned(),
                ),
            };
        }
        Ok(
            block_on_agent_session_registry_async(turso_session_for_root_session_id_any_project(
                &self.db_path,
                root_session_id.as_str(),
            ))?
            .map(|record| record.project_id.into_string()),
        )
    }
}
