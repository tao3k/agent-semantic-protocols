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
    let operation = match operation {
        crate::workspace_db_ipc::AgentSessionRegistryIpcOperation::Query {
            project_id,
            root_session_id,
            name,
        } => {
            return Ok(
                crate::workspace_db_ipc::AgentSessionRegistryIpcResult::Sessions {
                    sessions: registry
                        .query_sessions_local(
                            project_id,
                            root_session_id.map(Into::into),
                            name.map(Into::into),
                        )
                        .await?,
                },
            );
        }
        crate::workspace_db_ipc::AgentSessionRegistryIpcOperation::RecordHostExecutionObservation {
            observation,
        } => {
            return Ok(crate::workspace_db_ipc::AgentSessionRegistryIpcResult::Changed {
                changed: registry
                    .record_host_execution_observation_local(&observation)
                    .await?,
            });
        }
        operation => operation,
    };
    tokio::task::spawn_blocking(move || {
        use crate::workspace_db_ipc::{
            AgentSessionRegistryIpcOperation as Operation,
            AgentSessionRegistryIpcResult as IpcResult,
        };

        match operation {
            Operation::RegisterControlPlaneAgent { .. }
            | Operation::AdmitControlPlaneDelegation { .. }
            | Operation::ReadControlPlaneSnapshot { .. }
            | Operation::RecordHostExecutionObservation { .. } => Err(
                "session control-plane operation reached the blocking registry dispatcher"
                    .to_owned(),
            ),
            Operation::RecordHostLifecycleEvent { event } => {
                record_host_lifecycle_event(&registry, event)
            }
            Operation::RecordHostNonMatch { observation } => {
                registry.record_host_non_match_local(&observation)?;
                Ok(IpcResult::Changed { changed: true })
            }
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
                project_id: _,
                root_session_id: _,
                name: _,
            } => Err("agent-session query must use the async Runtime Server owner lane".to_owned()),
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
            Operation::DispatchLease {
                project_id,
                root_session_id,
                name,
                dispatch_identity,
            } => Ok(IpcResult::DispatchLease {
                lease: registry.dispatch_lease(
                    project_id,
                    root_session_id,
                    name,
                    dispatch_identity,
                )?,
            }),
            Operation::CompleteDispatch {
                project_id,
                root_session_id,
                name,
                dispatch_identity,
                command_digest,
                evidence_ref,
                now,
            } => Ok(IpcResult::DispatchCompleted {
                lease: registry.complete_dispatch(
                    crate::agent_session_registry::AgentSessionDispatchCompleteRequest {
                        project_id: &project_id,
                        root_session_id: &root_session_id,
                        name: &name,
                        dispatch_identity: &dispatch_identity,
                        command_digest: &command_digest,
                        evidence_ref: &evidence_ref,
                        now,
                    },
                )?,
            }),
        }
    })
    .await
    .map_err(|error| format!("agent-session registry owner task failed: {error}"))?
}

fn record_host_lifecycle_event(
    registry: &crate::AgentSessionRegistry,
    event: crate::workspace_db_ipc::AgentHostLifecycleEventIpc,
) -> Result<crate::workspace_db_ipc::AgentSessionRegistryIpcResult, String> {
    use crate::workspace_db_ipc::{
        AgentHostLifecycleEventKind as EventKind, AgentSessionRegistryIpcResult as IpcResult,
    };

    match event.kind {
        EventKind::Started => {
            if event.session_lifetime != "resident" {
                return Err(
                    "temporary-session-authority-required: Host lifecycle registration requires a persisted temporaryAuthority receipt"
                        .to_owned(),
                );
            }
            let existing = registry.session_by_name(
                event.project_id.clone(),
                event.root_session_id.clone(),
                event.platform_host_agent_name.clone(),
            )?;
            let generation = match existing.as_ref() {
                Some(current)
                    if current.session_id() == event.child_session_id
                        && current.is_routable_at(event.observed_at) =>
                {
                    let metadata: serde_json::Value =
                        serde_json::from_str(current.metadata_json()).map_err(|_| {
                            "host-event-replay-authority-missing: current namespace metadata is invalid"
                                .to_owned()
                        })?;
                    let exact_replay = metadata
                        .get("hostEventId")
                        .and_then(serde_json::Value::as_str)
                        == Some(event.host_event_id.as_str())
                        && metadata
                            .get("hostEventSequence")
                            .and_then(serde_json::Value::as_u64)
                            == Some(event.host_event_sequence)
                        && metadata
                            .get("payloadDigest")
                            .and_then(serde_json::Value::as_str)
                            == Some(event.payload_digest.as_str());
                    if !exact_replay {
                        return Err(format!(
                            "existing-host-namespace-requires-resume: namespace={} generation={}",
                            event.namespace_id, current.physical_generation,
                        ));
                    }
                    u64::try_from(current.physical_generation).unwrap_or(0)
                }
                Some(current) if current.session_id() == event.child_session_id => {
                    return Err(format!(
                        "existing-host-namespace-requires-resume: child={} generation={}",
                        current.session_id(),
                        current.physical_generation,
                    ));
                }
                Some(current) => {
                    return Err(format!(
                        "existing-host-namespace-identity-mismatch: namespace={} currentChild={} proposedChild={}",
                        event.namespace_id,
                        current.session_id(),
                        event.child_session_id,
                    ));
                }
                None => 1,
            };
            let registration_id = format!(
                "blake3-256:{}",
                blake3::hash(
                    format!(
                        "{}\0{}\0{}\0{}\0{}\0{}",
                        event.root_session_id,
                        event.child_session_id,
                        event.route_key,
                        event.profile_digest,
                        event.model_digest,
                        event.sandbox_mode,
                    )
                    .as_bytes(),
                )
                .to_hex()
            );
            let binding_id = format!(
                "blake3-256:{}",
                blake3::hash(format!("{registration_id}\0{generation}").as_bytes()).to_hex()
            );
            let host_binding = serde_json::json!({
                "schemaId": "agent.semantic-protocols.agent-session-host-binding",
                "schemaVersion": "1",
                "bindingId": binding_id,
                "registrationId": registration_id,
                "rootSessionId": event.root_session_id,
                "hostChildId": event.child_session_id,
                "hostTaskName": event.host_task_name,
                "agentInstanceId": event.child_session_id,
                "residentId": event.platform_host_agent_name,
                "routeKey": event.route_key,
                "profileId": event.profile_id,
                "profileDigest": event.profile_digest,
                "modelId": event.model,
                "modelDigest": event.model_digest,
                "sandboxMode": event.sandbox_mode,
                "sessionLifetime": event.session_lifetime,
                "generation": generation,
                "lifecycleState": "live",
                "routable": true,
                "evidenceSequence": generation,
                "temporaryAuthority": null,
                "namespaceId": event.namespace_id,
                "hostEventId": event.host_event_id,
                "hostEventSequence": event.host_event_sequence,
            });
            let metadata_json = serde_json::json!({
                "event": "subagent-start",
                "native": true,
                "platform": event.platform,
                "rootSessionId": event.root_session_id,
                "parentSessionId": event.parent_session_id,
                "childSessionId": event.child_session_id,
                "agentType": event.platform_host_agent_name,
                "profileDigest": event.profile_digest,
                "transcriptPath": event.transcript_path,
                "matchDecision": "matched",
                "payloadDigest": event.payload_digest,
                "namespaceId": event.namespace_id,
                "hostEventId": event.host_event_id,
                "hostEventSequence": event.host_event_sequence,
                "hostBinding": host_binding,
            })
            .to_string();
            let model_observation = Some(crate::agent_session_registry::AgentSessionModelObservationRef {
                    model: event.model.as_str(),
                    source: crate::agent_session_registry::AgentSessionModelObservationSource::CodexSubagentStart,
                    observed_at: event.observed_at,
                    evidence_ref: event.transcript_path.as_deref(),
                });
            let request = crate::AgentSessionRegisterRequest {
                project_id: event.project_id.clone().into(),
                root_session_id: event.root_session_id.clone().into(),
                session_id: event.child_session_id.clone().into(),
                message_target_id: Some(event.child_session_id.clone().into()),
                parent_session_id: Some(event.parent_session_id.into()),
                name: event.platform_host_agent_name.into(),
                role: event.role.into(),
                model_observation,
                status: "active".into(),
                expires_at: None,
                metadata_json: metadata_json.into(),
                now: event.observed_at,
            };
            let session = match existing {
                Some(current) => current,
                None => registry.register_session(request)?,
            };
            Ok(IpcResult::Registered { session })
        }
        EventKind::Resumed => {
            let current = registry
                .session_by_name(
                    event.project_id.clone(),
                    event.root_session_id.clone(),
                    event.platform_host_agent_name.clone(),
                )?
                .ok_or_else(|| {
                    "resume-requires-existing-host-namespace: namespace is absent".to_owned()
                })?;
            if current.session_id() != event.child_session_id {
                return Err(format!(
                    "resume-host-namespace-identity-mismatch: namespace={} currentChild={} proposedChild={}",
                    event.namespace_id,
                    current.session_id(),
                    event.child_session_id,
                ));
            }
            Ok(IpcResult::Changed {
                changed: registry.update_session_status(
                    event.project_id,
                    event.child_session_id,
                    "active".to_owned(),
                    event.observed_at,
                )?,
            })
        }
        EventKind::Stopped => {
            let current = registry
                .session_by_id(event.project_id.clone(), event.child_session_id.clone())?
                .ok_or_else(|| {
                    "host-binding-terminal-instance-missing: exact child binding is unavailable"
                        .to_owned()
                })?;
            let binding: serde_json::Value = serde_json::from_str(current.metadata_json())
                .ok()
                .and_then(|metadata: serde_json::Value| metadata.get("hostBinding").cloned())
                .ok_or_else(|| {
                    "unbound-matched-child: exact Host binding receipt is unavailable".to_owned()
                })?;
            let exact = binding
                .get("rootSessionId")
                .and_then(serde_json::Value::as_str)
                == Some(event.root_session_id.as_str())
                && binding
                    .get("hostChildId")
                    .and_then(serde_json::Value::as_str)
                    == Some(event.child_session_id.as_str())
                && binding.get("routeKey").and_then(serde_json::Value::as_str)
                    == Some(event.route_key.as_str())
                && binding
                    .get("profileDigest")
                    .and_then(serde_json::Value::as_str)
                    == Some(event.profile_digest.as_str())
                && binding
                    .get("modelDigest")
                    .and_then(serde_json::Value::as_str)
                    == Some(event.model_digest.as_str())
                && binding
                    .get("sandboxMode")
                    .and_then(serde_json::Value::as_str)
                    == Some(event.sandbox_mode.as_str());
            if !exact {
                return Err("host-binding-terminal-identity-drift: stop evidence does not exact-match the live instance".to_owned());
            }
            Ok(IpcResult::Changed {
                changed: registry.update_session_status(
                    event.project_id,
                    event.child_session_id,
                    "stopped".to_owned(),
                    event.observed_at,
                )?,
            })
        }
        EventKind::Achieved => Ok(IpcResult::Changed {
            changed: registry.update_session_status(
                event.project_id,
                event.child_session_id,
                "achieved".to_owned(),
                event.observed_at,
            )?,
        }),
    }
}

#[cfg(test)]
#[path = "../tests/unit/workspace_db_ipc_server_agent_session_registry.rs"]
mod tests;
