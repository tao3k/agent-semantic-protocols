//! Synchronous public registry operations over the Turso-owned core.

use super::AgentSessionRegistry;
use super::host_execution::turso_observe_host_execution;
use super::storage::{
    block_on_agent_session_registry_async, turso_claim_resident_session, turso_delete_session,
    turso_query_all_sessions, turso_query_sessions, turso_record_host_non_match,
    turso_record_tool_event, turso_refresh_expired_sessions, turso_register_session,
    turso_session_by_id, turso_session_by_id_any_project, turso_session_by_name,
    turso_session_for_root_session_id_any_project, turso_set_archived_status,
    turso_update_session_status,
};
use crate::agent_session_registry::types::{
    AGENT_SESSION_STATUS_ACTIVE, AGENT_SESSION_STATUS_ARCHIVED, AGENT_SESSION_STATUS_INVALID,
    AgentSessionDispatchClaimRequest, AgentSessionDispatchClaimResult,
    AgentSessionDispatchCompleteRequest, AgentSessionDispatchLeaseRecord, AgentSessionId,
    AgentSessionLookupRequest, AgentSessionProjectId, AgentSessionRecord,
    AgentSessionRegisterRequest, AgentSessionResidentName, AgentSessionRootSessionId,
    AgentSessionStatus, AgentSessionToolEventRequest, agent_session_unix_timestamp,
};

impl AgentSessionRegistry {
    pub(crate) async fn record_host_execution_observation_local(
        &self,
        observation: &crate::workspace_db_ipc::AgentHostExecutionObservationIpc,
    ) -> Result<bool, String> {
        let current = self
            .query_sessions_local(
                observation.project_id.clone(),
                Some(observation.root_session_id.clone().into()),
                Some(observation.platform_host_agent_name.clone().into()),
            )
            .await?
            .into_iter()
            .next()
            .ok_or_else(|| "host-execution-observation-requires-existing-namespace".to_owned())?;
        if current.session_id() != observation.child_session_id {
            return Err(format!(
                "host-execution-observation-identity-mismatch: currentChild={} observedChild={}",
                current.session_id(),
                observation.child_session_id
            ));
        }
        if matches!(current.status(), "achieved" | AGENT_SESSION_STATUS_INVALID) {
            return Err(format!(
                "host-execution-observation-terminal-namespace: status={}",
                current.status()
            ));
        }
        let mut metadata = serde_json::from_str::<serde_json::Value>(current.metadata_json())
            .map_err(|_| "host-execution-observation-invalid-metadata".to_owned())?;
        let binding = metadata
            .get_mut("hostBinding")
            .and_then(serde_json::Value::as_object_mut)
            .ok_or_else(|| "host-execution-observation-requires-host-binding".to_owned())?;
        binding.insert(
            "hostChildId".to_owned(),
            observation.child_session_id.clone().into(),
        );
        binding.insert(
            "agentInstanceId".to_owned(),
            observation.child_session_id.clone().into(),
        );
        binding.insert("lifecycleState".to_owned(), "live".into());
        binding.insert("routable".to_owned(), true.into());
        metadata["lastExecutionObservation"] = serde_json::json!({
            "observationId": observation.observation_id,
            "transcriptPath": observation.transcript_path,
            "observedAt": observation.observed_at,
        });
        turso_observe_host_execution(&self.db_path, observation, &metadata.to_string()).await
    }

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
        let endpoint = crate::read_runtime_server_endpoint(&state.state_home)?
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

    pub fn register_session(
        &self,
        request: AgentSessionRegisterRequest<'_>,
    ) -> Result<AgentSessionRecord, String> {
        if self.session_is_retired(request.project_id.as_str(), request.session_id.as_str())? {
            return Err(format!(
                "retired physical session generation cannot be registered again: projectId={} sessionId={}",
                request.project_id, request.session_id
            ));
        }
        if let Some(result) = self.runtime_operation(
            crate::workspace_db_ipc::AgentSessionRegistryIpcOperation::Register {
                request: crate::workspace_db_ipc::AgentSessionRegisterIpcRequest {
                    project_id: request.project_id.as_str().to_owned(),
                    root_session_id: request.root_session_id.as_str().to_owned(),
                    session_id: request.session_id.as_str().to_owned(),
                    message_target_id: request
                        .message_target_id
                        .as_ref()
                        .map(|value| value.as_str().to_owned()),
                    parent_session_id: request
                        .parent_session_id
                        .as_ref()
                        .map(|value| value.as_str().to_owned()),
                    name: request.name.as_str().to_owned(),
                    role: request.role.as_str().to_owned(),
                    model_observation: request.model_observation.map(|observation| {
                        crate::workspace_db_ipc::AgentSessionModelObservationIpc {
                            model: observation.model.to_owned(),
                            source: observation.source.as_str().to_owned(),
                            observed_at: observation.observed_at,
                            evidence_ref: observation.evidence_ref.map(str::to_owned),
                        }
                    }),
                    status: request.status.as_str().to_owned(),
                    expires_at: request.expires_at,
                    metadata_json: request.metadata_json.as_str().to_owned(),
                    now: request.now,
                },
            },
        )? {
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
    pub fn record_host_lifecycle_event(
        &self,
        event: crate::workspace_db_ipc::AgentHostLifecycleEventIpc,
    ) -> Result<crate::workspace_db_ipc::AgentSessionRegistryIpcResult, String> {
        self.runtime_operation(
            crate::workspace_db_ipc::AgentSessionRegistryIpcOperation::RecordHostLifecycleEvent {
                event,
            },
        )?
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

    pub fn record_host_non_match(
        &self,
        observation: crate::workspace_db_ipc::AgentHostNonMatchIpc,
    ) -> Result<crate::workspace_db_ipc::AgentSessionRegistryIpcResult, String> {
        self.runtime_operation(
            crate::workspace_db_ipc::AgentSessionRegistryIpcOperation::RecordHostNonMatch {
                observation,
            },
        )?
        .ok_or_else(|| "Host match decisions require the Runtime Server registry proxy".to_owned())
    }

    pub(crate) fn record_host_non_match_local(
        &self,
        observation: &crate::workspace_db_ipc::AgentHostNonMatchIpc,
    ) -> Result<(), String> {
        block_on_agent_session_registry_async(turso_record_host_non_match(
            &self.db_path,
            observation,
        ))
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
                    project_id,
                    root_session_id,
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

    /// Claim a resident route without replacing the child that already owns it.
    pub fn claim_resident_session(
        &self,
        request: AgentSessionRegisterRequest<'_>,
    ) -> Result<AgentSessionRecord, String> {
        block_on_agent_session_registry_async(turso_claim_resident_session(&self.db_path, request))
    }

    /// Return registered sessions for one project, optionally narrowed by root session and name.
    pub fn query_sessions(
        &self,
        project_id: impl Into<AgentSessionProjectId>,
        root_session_id: Option<AgentSessionRootSessionId>,
        name: Option<AgentSessionResidentName>,
    ) -> Result<Vec<AgentSessionRecord>, String> {
        let project_id = project_id.into();
        if let Some(result) = self.runtime_operation(
            crate::workspace_db_ipc::AgentSessionRegistryIpcOperation::Query {
                project_id: project_id.as_str().to_owned(),
                root_session_id: root_session_id
                    .as_ref()
                    .map(|value| value.as_str().to_owned()),
                name: name.as_ref().map(|value| value.as_str().to_owned()),
            },
        )? {
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
    pub(crate) async fn query_sessions_local(
        &self,
        project_id: impl Into<AgentSessionProjectId>,
        root_session_id: Option<AgentSessionRootSessionId>,
        name: Option<AgentSessionResidentName>,
    ) -> Result<Vec<AgentSessionRecord>, String> {
        let project_id = project_id.into();
        turso_query_sessions(
            &self.db_path,
            project_id.as_str(),
            root_session_id
                .as_ref()
                .map(AgentSessionRootSessionId::as_str),
            name.as_ref().map(AgentSessionResidentName::as_str),
        )
        .await
    }

    /// Return one registered session by its concrete session id.
    pub fn session_by_id(
        &self,
        project_id: impl Into<AgentSessionProjectId>,
        session_id: impl Into<AgentSessionId>,
    ) -> Result<Option<AgentSessionRecord>, String> {
        let project_id = project_id.into();
        let session_id = session_id.into();
        if let Some(result) = self.runtime_operation(
            crate::workspace_db_ipc::AgentSessionRegistryIpcOperation::SessionById {
                project_id: project_id.as_str().to_owned(),
                session_id: session_id.as_str().to_owned(),
            },
        )? {
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
    pub fn session_by_name(
        &self,
        project_id: impl Into<AgentSessionProjectId>,
        root_session_id: impl Into<AgentSessionRootSessionId>,
        name: impl Into<AgentSessionResidentName>,
    ) -> Result<Option<AgentSessionRecord>, String> {
        let project_id = project_id.into();
        let root_session_id = root_session_id.into();
        let name = name.into();
        if let Some(result) = self.runtime_operation(
            crate::workspace_db_ipc::AgentSessionRegistryIpcOperation::SessionByName {
                project_id: project_id.as_str().to_owned(),
                root_session_id: root_session_id.as_str().to_owned(),
                name: name.as_str().to_owned(),
            },
        )? {
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
    pub fn claim_dispatch(
        &self,
        request: AgentSessionDispatchClaimRequest<'_>,
    ) -> Result<AgentSessionDispatchClaimResult, String> {
        if let Some(result) = self.runtime_operation(
            crate::workspace_db_ipc::AgentSessionRegistryIpcOperation::ClaimDispatch {
                project_id: request.project_id.to_owned(),
                root_session_id: request.root_session_id.to_owned(),
                name: request.name.to_owned(),
                dispatch_identity: request.dispatch_identity.to_owned(),
                command_digest: request.command_digest.to_owned(),
                delivery_target_override: request.delivery_target_override.map(str::to_owned),
                now: request.now,
            },
        )? {
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
    pub fn complete_dispatch(
        &self,
        request: AgentSessionDispatchCompleteRequest<'_>,
    ) -> Result<AgentSessionDispatchLeaseRecord, String> {
        if let Some(result) = self.runtime_operation(
            crate::workspace_db_ipc::AgentSessionRegistryIpcOperation::CompleteDispatch {
                project_id: request.project_id.to_owned(),
                root_session_id: request.root_session_id.to_owned(),
                name: request.name.to_owned(),
                dispatch_identity: request.dispatch_identity.to_owned(),
                command_digest: request.command_digest.to_owned(),
                evidence_ref: request.evidence_ref.to_owned(),
                now: request.now,
            },
        )? {
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
    pub fn lookup_session(
        &self,
        request: AgentSessionLookupRequest,
    ) -> Result<Option<AgentSessionRecord>, String> {
        if let Some(session_id) = request.session_id.as_ref() {
            return self.session_by_id(request.project_id.clone(), session_id.clone());
        }
        if let (Some(root_session_id), Some(name)) =
            (request.root_session_id.as_ref(), request.name.as_ref())
        {
            return self.session_by_name(
                request.project_id.clone(),
                root_session_id.clone(),
                name.clone(),
            );
        }
        let sessions =
            self.query_sessions(request.project_id, request.root_session_id, request.name)?;
        Ok(sessions.into_iter().next())
    }

    /// Update one session row to the supplied routing status.
    pub fn update_session_status(
        &self,
        project_id: impl Into<AgentSessionProjectId>,
        session_id: impl Into<AgentSessionId>,
        status: impl Into<AgentSessionStatus>,
        now: i64,
    ) -> Result<bool, String> {
        let project_id = project_id.into();
        let session_id = session_id.into();
        let status = status.into();
        if let Some(result) = self.runtime_operation(
            crate::workspace_db_ipc::AgentSessionRegistryIpcOperation::UpdateStatus {
                project_id: project_id.as_str().to_owned(),
                session_id: session_id.as_str().to_owned(),
                status: status.as_str().to_owned(),
                now,
            },
        )? {
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
    pub fn mark_session_invalid(
        &self,
        project_id: impl Into<AgentSessionProjectId>,
        session_id: impl Into<AgentSessionId>,
        now: i64,
    ) -> Result<bool, String> {
        let project_id = project_id.into();
        let session_id = session_id.into();
        self.update_session_status(project_id, session_id, AGENT_SESSION_STATUS_INVALID, now)
    }

    /// Archive one session row.
    pub fn archive_session(
        &self,
        project_id: impl Into<AgentSessionProjectId>,
        session_id: impl Into<AgentSessionId>,
        now: i64,
    ) -> Result<bool, String> {
        let project_id = project_id.into();
        let session_id = session_id.into();
        if let Some(result) = self.runtime_operation(
            crate::workspace_db_ipc::AgentSessionRegistryIpcOperation::SetArchivedStatus {
                project_id: project_id.as_str().to_owned(),
                session_id: session_id.as_str().to_owned(),
                archived: true,
                now,
            },
        )? {
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
    pub fn unarchive_session(
        &self,
        project_id: impl Into<AgentSessionProjectId>,
        session_id: impl Into<AgentSessionId>,
        now: i64,
    ) -> Result<bool, String> {
        let project_id = project_id.into();
        let session_id = session_id.into();
        if let Some(result) = self.runtime_operation(
            crate::workspace_db_ipc::AgentSessionRegistryIpcOperation::SetArchivedStatus {
                project_id: project_id.as_str().to_owned(),
                session_id: session_id.as_str().to_owned(),
                archived: false,
                now,
            },
        )? {
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
    pub fn session_is_retired(
        &self,
        project_id: impl Into<AgentSessionProjectId>,
        session_id: impl Into<AgentSessionId>,
    ) -> Result<bool, String> {
        let project_id = project_id.into();
        let session_id = session_id.into();
        if let Some(result) = self.runtime_operation(
            crate::workspace_db_ipc::AgentSessionRegistryIpcOperation::SessionIsRetired {
                project_id: project_id.as_str().to_owned(),
                session_id: session_id.as_str().to_owned(),
            },
        )? {
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
    pub fn refresh_expired_sessions(&self) -> Result<(), String> {
        if let Some(result) = self.runtime_operation(
            crate::workspace_db_ipc::AgentSessionRegistryIpcOperation::RefreshExpired,
        )? {
            return match result {
                crate::workspace_db_ipc::AgentSessionRegistryIpcResult::Refreshed => Ok(()),
                _ => Err("Runtime Server returned an unexpected session refresh result".to_owned()),
            };
        }
        let now = agent_session_unix_timestamp()?;
        block_on_agent_session_registry_async(turso_refresh_expired_sessions(&self.db_path, now))
    }

    /// Return one registered session by its concrete session id across all projects.
    pub fn session_by_id_any_project(
        &self,
        session_id: impl Into<AgentSessionId>,
    ) -> Result<Option<AgentSessionRecord>, String> {
        let session_id = session_id.into();
        if let Some(result) = self.runtime_operation(
            crate::workspace_db_ipc::AgentSessionRegistryIpcOperation::SessionByIdAnyProject {
                session_id: session_id.as_str().to_owned(),
            },
        )? {
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
    pub fn project_id_for_root_session_id(
        &self,
        root_session_id: impl Into<AgentSessionRootSessionId>,
    ) -> Result<Option<String>, String> {
        let root_session_id = root_session_id.into();
        if let Some(result) = self.runtime_operation(
            crate::workspace_db_ipc::AgentSessionRegistryIpcOperation::ProjectIdForRootSessionId {
                root_session_id: root_session_id.as_str().to_owned(),
            },
        )? {
            return match result {
                crate::workspace_db_ipc::AgentSessionRegistryIpcResult::ProjectId {
                    project_id,
                } => Ok(project_id),
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
