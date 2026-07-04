use agent_semantic_client_db::{
    AGENT_SESSION_STATUS_INVALID, AgentSessionLookupRequest, AgentSessionRecord,
    AgentSessionRegisterRequest, AgentSessionRegistry, agent_session_normalized_metadata_json,
    agent_session_unix_timestamp,
};
use agent_semantic_runtime::{
    CodexRolloutSessionIndex, agent_session_registration_identity,
    agent_session_runtime_status_snapshot, codex_rollout_session_index,
};

use super::agent_session_registry_args::SessionArgs;
use super::agent_session_registry_render::{
    ActivitySnapshotShort, SessionLifecycleIndex, SessionStatusReport, escape_field,
    print_json_report, print_reuse_miss, print_reuse_session, print_session_row,
    print_status_report,
};
use super::agent_session_registry_rollout_activity::rollout_activity_report;
use super::agent_session_registry_state::{
    current_project_session_scope_id, current_recall_session_id, project_session_scope_id,
    required_non_empty, resolved_root_session_id, session_record_validation_allows_routing,
};
use super::agent_session_registry_validation::{
    validate_recent_session_profile, validate_session_profile,
};
use std::path::Path;

pub(super) fn register_session(
    registry: &AgentSessionRegistry,
    args: &SessionArgs,
) -> Result<(), String> {
    let project_id = current_project_session_scope_id()?;
    let identity = agent_session_registration_identity(
        args.child_session_id.as_deref(),
        args.root_session_id.as_deref(),
    )?;
    let session_id = identity.session_id;
    let now = agent_session_unix_timestamp()?;
    let root_session_id = identity.root_session_id;
    let name = required_non_empty(args.name.as_deref(), "--name")?.to_string();
    let role = args.role.as_deref().unwrap_or("agent").to_string();
    let status = args.status.as_deref().unwrap_or("active").to_string();
    let validation = if args.replace {
        validate_session_profile(&session_id, &root_session_id, &name, &role, now)?
    } else {
        validate_recent_session_profile(&session_id, &root_session_id, &name, &role, now)?
    };
    let metadata_json =
        agent_session_normalized_metadata_json(args.metadata_json.as_deref(), &validation)?;
    if validation.status == "failed" {
        let _ = registry.mark_session_invalid(&project_id, &session_id, now);
        let _ = registry.register_session(AgentSessionRegisterRequest {
            project_id: &project_id,
            root_session_id: &root_session_id,
            session_id: &session_id,
            parent_session_id: args.parent_session_id.as_deref(),
            name: &name,
            role: &role,
            model: args.model.as_deref(),
            status: AGENT_SESSION_STATUS_INVALID,
            expires_at: args.expires_at,
            metadata_json: &metadata_json,
            now,
        });
        return Err(format!(
            "agent session validation failed: {}.\nblockedState=validation-failed-or-non-routable-child\nnextAction=destroy-invalid-child-and-create-configured-child\nstatusCommand=asp agent session status --name {name} --json\nClose/delete this child session and create a fresh child from the configured Codex agent before registering again.",
            validation.reason
        ));
    }
    if !args.replace
        && let Some(existing) = registry.lookup_session(AgentSessionLookupRequest {
            project_id: &project_id,
            session_id: None,
            root_session_id: Some(&root_session_id),
            name: Some(&name),
        })?
        && existing.session_id != session_id
        && existing.is_routable_at(now)
        && session_record_validation_allows_routing(registry, &existing, now)?
    {
        return print_reuse_session(
            registry.db_path(),
            Some(&root_session_id),
            existing,
            args.json,
        );
    }

    let record = registry.register_session(AgentSessionRegisterRequest {
        project_id: &project_id,
        root_session_id: &root_session_id,
        session_id: &session_id,
        parent_session_id: args.parent_session_id.as_deref(),
        name: &name,
        role: &role,
        model: args.model.as_deref(),
        status: &status,
        expires_at: args.expires_at,
        metadata_json: &metadata_json,
        now,
    })?;
    if args.json {
        print_json_report(registry.db_path(), Some(&root_session_id), vec![record])
    } else {
        println!(
            "[agent-session-register] owner=rust rootSession=\"{}\" session=\"{}\" name=\"{}\" role=\"{}\" status=\"{}\" db=\"{}\"",
            escape_field(&root_session_id),
            escape_field(&session_id),
            escape_field(&name),
            escape_field(&role),
            escape_field(&status),
            registry.db_path().display()
        );
        Ok(())
    }
}

pub(super) fn lifecycle_audit_session(
    registry: &AgentSessionRegistry,
    args: &SessionArgs,
) -> Result<(), String> {
    let project_id = current_project_session_scope_id()?;
    let root_filter = if args.all {
        None
    } else {
        match args.root_session_id.clone() {
            Some(root_session_id) => Some(root_session_id),
            None => current_recall_session_id(registry)?,
        }
    };
    registry.refresh_expired_sessions()?;
    let sessions =
        registry.query_sessions(&project_id, root_filter.as_deref(), args.name.as_deref())?;
    let (rollout_session_index, rollout_index_error) =
        match root_filter.as_deref().map(codex_rollout_session_index) {
            Some(Ok(index)) => (index, None),
            Some(Err(error)) => (None, Some(error)),
            None => (None, None),
        };
    let report = lifecycle_audit_report(
        registry,
        project_id,
        root_filter,
        sessions,
        rollout_session_index,
        rollout_index_error,
    )?;
    if args.json {
        let body = serde_json::to_string_pretty(&report)
            .map_err(|error| format!("serialize lifecycle audit report: {error}"))?;
        println!("{body}");
    } else {
        println!(
            "[agent-session-lifecycle-audit] owner=rust rootSession={} registrySessions={} rolloutSessions={} rolloutOnlySessions={} missingRegisteredRollouts={} missingRollouts={} db=\"{}\"",
            report["rootSessionId"]
                .as_str()
                .map(|value| format!("\"{}\"", escape_field(value)))
                .unwrap_or_else(|| "\"*\"".to_string()),
            report["summary"]["registrySessionCount"]
                .as_u64()
                .unwrap_or(0),
            report["summary"]["rolloutSessionCount"]
                .as_u64()
                .unwrap_or(0),
            report["summary"]["rolloutOnlySessionCount"]
                .as_u64()
                .unwrap_or(0),
            report["summary"]["missingRegisteredRolloutCount"]
                .as_u64()
                .unwrap_or(0),
            report["summary"]["missingRolloutCount"]
                .as_u64()
                .unwrap_or(0),
            registry.db_path().display(),
        );
        println!(
            "hint: rerun with `asp agent session lifecycle audit --json` for per-session evidence"
        );
    }
    Ok(())
}

fn lifecycle_audit_report(
    registry: &AgentSessionRegistry,
    project_id: String,
    root_filter: Option<String>,
    sessions: Vec<AgentSessionRecord>,
    rollout_session_index: Option<CodexRolloutSessionIndex>,
    rollout_index_error: Option<String>,
) -> Result<serde_json::Value, String> {
    let registered_session_ids: std::collections::BTreeSet<String> = sessions
        .iter()
        .map(|session| session.session_id.clone())
        .collect();
    let registry_sessions: Vec<serde_json::Value> = sessions
        .iter()
        .map(lifecycle_registry_session_entry)
        .collect();
    let mut rollout_session_ids = std::collections::BTreeSet::<String>::new();
    let mut registered_rollout_sessions = Vec::<serde_json::Value>::new();
    let mut rollout_only_sessions = Vec::<serde_json::Value>::new();
    let mut rollout_activity_count = 0_usize;
    let mut missing_rollout_count = 0_usize;
    let mut scanned_rollout_count = 0_usize;
    let mut skipped_rollout_count = 0_usize;
    let mut missing_rollout_by_session = serde_json::json!({});

    if let Some(index) = rollout_session_index.as_ref() {
        rollout_activity_count = index.activity_by_session.len();
        missing_rollout_count = index.missing_rollout_by_session.len();
        scanned_rollout_count = index.scanned_rollout_count;
        skipped_rollout_count = index.skipped_rollout_count;
        missing_rollout_by_session = serde_json::to_value(&index.missing_rollout_by_session)
            .map_err(|error| format!("serialize missing rollout map: {error}"))?;
        for record in &index.records {
            let Some((session_id, entry)) = lifecycle_rollout_session_entry(index, record)? else {
                continue;
            };
            rollout_session_ids.insert(session_id.clone());
            if registered_session_ids.contains(&session_id) {
                registered_rollout_sessions.push(entry);
            } else {
                rollout_only_sessions.push(entry);
            }
        }
    }

    let missing_registered_rollout_sessions: Vec<serde_json::Value> = sessions
        .iter()
        .filter(|session| !rollout_session_ids.contains(&session.session_id))
        .map(lifecycle_registry_session_entry)
        .collect();

    Ok(serde_json::json!({
        "owner": "rust",
        "action": "agent-session-lifecycle-audit",
        "dbPath": registry.db_path(),
        "projectId": project_id,
        "rootSessionId": root_filter,
        "summary": {
            "registrySessionCount": registry_sessions.len(),
            "rolloutSessionCount": rollout_session_ids.len(),
            "rolloutActivityCount": rollout_activity_count,
            "registeredRolloutSessionCount": registered_rollout_sessions.len(),
            "rolloutOnlySessionCount": rollout_only_sessions.len(),
            "missingRegisteredRolloutCount": missing_registered_rollout_sessions.len(),
            "missingRolloutCount": missing_rollout_count,
            "scannedRolloutCount": scanned_rollout_count,
            "skippedRolloutCount": skipped_rollout_count,
        },
        "rolloutIndexError": rollout_index_error,
        "registrySessions": registry_sessions,
        "registeredRolloutSessions": registered_rollout_sessions,
        "rolloutOnlySessions": rollout_only_sessions,
        "missingRegisteredRolloutSessions": missing_registered_rollout_sessions,
        "missingRolloutBySession": missing_rollout_by_session,
    }))
}

fn lifecycle_registry_session_entry(session: &AgentSessionRecord) -> serde_json::Value {
    serde_json::json!({
        "projectId": session.project_id,
        "rootSessionId": session.root_session_id,
        "sessionId": session.session_id,
        "parentSessionId": session.parent_session_id,
        "name": session.name,
        "role": session.role,
        "model": session.model,
        "status": session.status,
    })
}

fn lifecycle_rollout_session_entry(
    index: &CodexRolloutSessionIndex,
    record: &impl serde::Serialize,
) -> Result<Option<(String, serde_json::Value)>, String> {
    let mut entry = serde_json::to_value(record)
        .map_err(|error| format!("serialize rollout session record: {error}"))?;
    let Some(session_id) = entry
        .get("sessionId")
        .and_then(serde_json::Value::as_str)
        .map(ToOwned::to_owned)
    else {
        return Ok(None);
    };
    if let Some(object) = entry.as_object_mut() {
        if let Some(activity) = index.activity_by_session.get(&session_id) {
            object.insert(
                "rolloutStatus".to_string(),
                serde_json::json!(activity.status),
            );
            object.insert(
                "lastHeartbeatAt".to_string(),
                serde_json::json!(activity.last_heartbeat_at),
            );
            object.insert(
                "lastTerminalEvent".to_string(),
                serde_json::json!(activity.last_terminal_event),
            );
        }
    }
    Ok(Some((session_id, entry)))
}

pub(super) fn list_sessions(
    registry: &AgentSessionRegistry,
    args: &SessionArgs,
) -> Result<(), String> {
    let project_id = current_project_session_scope_id()?;
    let root_filter = if args.all {
        None
    } else {
        match args.root_session_id.clone() {
            Some(root_session_id) => Some(root_session_id),
            None => current_recall_session_id(registry)?,
        }
    };
    registry.refresh_expired_sessions()?;
    let mut sessions =
        registry.query_sessions(&project_id, root_filter.as_deref(), args.name.as_deref())?;
    if args.active {
        let now = agent_session_unix_timestamp()?;
        sessions.retain(|session| session.is_routable_at(now));
    }
    if args.json {
        return print_json_report(registry.db_path(), root_filter.as_deref(), sessions);
    }
    println!(
        "[agent-session-list] owner=rust rootSession={} sessions={} db=\"{}\"",
        root_filter
            .as_deref()
            .map(|value| format!("\"{}\"", escape_field(value)))
            .unwrap_or_else(|| "\"*\"".to_string()),
        sessions.len(),
        registry.db_path().display()
    );
    for session in sessions {
        print_session_row(&session);
    }
    Ok(())
}

pub(super) fn reuse_session(
    registry: &AgentSessionRegistry,
    args: &SessionArgs,
) -> Result<(), String> {
    let project_id = current_project_session_scope_id()?;
    registry.refresh_expired_sessions()?;
    let root_session_id = resolved_root_session_id(registry, args.root_session_id.as_deref())?
        .ok_or_else(|| {
            "asp agent session reuse requires --root-session-id or agent session env".to_string()
        })?;
    let name = required_non_empty(args.name.as_deref(), "--name")?;
    let Some(record) = registry.lookup_session(AgentSessionLookupRequest {
        project_id: &project_id,
        session_id: None,
        root_session_id: Some(&root_session_id),
        name: Some(name),
    })?
    else {
        return print_reuse_miss(
            registry.db_path(),
            Some(&root_session_id),
            name,
            "missing",
            args.json,
        );
    };
    let now = agent_session_unix_timestamp()?;
    if record.is_routable_at(now) {
        let validation = validate_session_profile(
            &record.session_id,
            &record.root_session_id,
            &record.name,
            &record.role,
            now,
        )?;
        if validation.status == "failed" {
            let _ = registry.mark_session_invalid(&project_id, &record.session_id, now);
            return print_reuse_miss(
                registry.db_path(),
                Some(&root_session_id),
                name,
                &validation.reason,
                args.json,
            );
        }
        print_reuse_session(
            registry.db_path(),
            Some(&root_session_id),
            record,
            args.json,
        )
    } else {
        let reason = record.status.clone();
        print_reuse_miss(
            registry.db_path(),
            Some(&root_session_id),
            name,
            &reason,
            args.json,
        )
    }
}

pub(super) fn show_session(
    registry: &AgentSessionRegistry,
    args: &SessionArgs,
) -> Result<(), String> {
    let project_id = current_project_session_scope_id()?;
    registry.refresh_expired_sessions()?;
    let name = if args.child_session_id.is_some() {
        None
    } else {
        Some(required_non_empty(
            args.name.as_deref(),
            "--name or --child-session-id",
        )?)
    };
    let root_session_id = if args.child_session_id.is_some() {
        None
    } else {
        Some(
            resolved_root_session_id(registry, args.root_session_id.as_deref())?.ok_or_else(
                || {
                    "asp agent session show --name requires --root-session-id or agent session env"
                        .to_string()
                },
            )?,
        )
    };
    let record = registry
        .lookup_session(AgentSessionLookupRequest {
            project_id: &project_id,
            session_id: args.child_session_id.as_deref(),
            root_session_id: root_session_id.as_deref(),
            name,
        })?
        .ok_or_else(|| "session registry entry not found".to_string())?;

    if args.json {
        let root_session_id = record.root_session_id.clone();
        print_json_report(registry.db_path(), Some(&root_session_id), vec![record])
    } else {
        println!(
            "[agent-session-show] owner=rust rootSession=\"{}\" sessions=1 db=\"{}\"",
            escape_field(&record.root_session_id),
            registry.db_path().display()
        );
        print_session_row(&record);
        Ok(())
    }
}

pub(super) fn status_session(
    registry: &AgentSessionRegistry,
    args: &SessionArgs,
    project_root: &Path,
) -> Result<(), String> {
    let project_id = project_session_scope_id(project_root);
    registry.refresh_expired_sessions()?;
    let root_session_id = resolved_root_session_id(registry, args.root_session_id.as_deref())?;
    let name = args.name.clone();
    let record = registry.lookup_session(AgentSessionLookupRequest {
        project_id: &project_id,
        session_id: args.child_session_id.as_deref(),
        root_session_id: root_session_id.as_deref(),
        name: name.as_deref(),
    })?;
    let now = agent_session_unix_timestamp()?;
    let routable = record
        .as_ref()
        .is_some_and(|session| session.is_routable_at(now));
    let validation = record
        .as_ref()
        .map(|session| {
            validate_session_profile(
                &session.session_id,
                &session.root_session_id,
                &session.name,
                &session.role,
                now,
            )
        })
        .transpose()?;
    let validation_allows_routing = validation.as_ref().is_none_or(|validation| {
        matches!(validation.status.as_str(), "passed" | "warning" | "skipped")
    });
    let routable = routable && validation_allows_routing;
    let registry_status = record
        .as_ref()
        .map(|session| session.status.clone())
        .unwrap_or_else(|| "missing".to_string());
    let host_thread_id = record
        .as_ref()
        .map(|session| session.session_id.as_str())
        .or_else(|| root_session_id.as_deref());
    let runtime_status = agent_session_runtime_status_snapshot(
        project_root,
        now,
        args.artifact_stale_after_seconds,
        host_thread_id,
        record.is_some(),
        routable,
    )?;
    let (rollout_session_index, rollout_session_index_error) =
        match root_session_id.as_deref().map(codex_rollout_session_index) {
            Some(Ok(index)) => (index, None),
            Some(Err(error)) => (None, Some(error)),
            None => (None, None),
        };
    let rollout_activity = validation
        .as_ref()
        .and_then(|validation| validation.rollout_path.as_deref())
        .map(|rollout_path| rollout_activity_report(Path::new(rollout_path), now));
    let session_lifecycle_index = Some(session_lifecycle_index(
        root_session_id.as_deref(),
        name.as_deref(),
        record.as_ref(),
        &registry_status,
        routable,
        rollout_session_index.as_ref(),
        rollout_session_index_error.as_deref(),
    ));
    let activity_snapshot_short = Some(activity_snapshot_short(
        root_session_id.as_deref(),
        record.as_ref(),
        &registry_status,
        &runtime_status.host_status,
        &runtime_status.health_status,
        &runtime_status.next_action,
        rollout_activity.as_ref(),
        rollout_session_index.as_ref(),
        rollout_session_index_error.as_deref(),
    ));
    let (host_thread_existence, host_thread_existence_reason) =
        host_thread_existence_snapshot(runtime_status.host_thread_id.as_deref());
    let multi_agent_child_state = multi_agent_child_state_snapshot(rollout_activity.as_ref());
    print_status_report(
        SessionStatusReport {
            owner: "rust",
            db_path: registry.db_path().display().to_string(),
            root_session_id,
            name,
            session: record,
            registry_status,
            routable,
            validation_status: validation
                .as_ref()
                .map(|validation| validation.status.clone())
                .unwrap_or_else(|| "missing-registry".to_string()),
            validation_reason: validation
                .as_ref()
                .map(|validation| validation.reason.clone())
                .unwrap_or_else(|| "session registry entry not found".to_string()),
            validation,
            rollout_session_index,
            rollout_activity,
            session_lifecycle_index,
            activity_snapshot_short,
            host_client: runtime_status.host_client,
            host_thread_id: runtime_status.host_thread_id,
            host_status_source: runtime_status.host_status_source,
            host_status: runtime_status.host_status,
            host_status_reason: runtime_status.host_status_reason,
            host_thread_existence,
            host_thread_existence_reason,
            multi_agent_child_state,
            host_raw_status: runtime_status.host_raw_status,
            health_status: runtime_status.health_status,
            timeout_semantics: runtime_status.timeout_semantics,
            duplicate_worker_allowed: runtime_status.duplicate_worker_allowed,
            artifacts_dir: runtime_status.artifacts_dir,
            artifact_status: runtime_status.artifact_status,
            artifact_stale_after_seconds: runtime_status.artifact_stale_after_seconds,
            last_artifact_updated_at: runtime_status.last_artifact_updated_at,
            artifact_age_seconds: runtime_status.artifact_age_seconds,
            last_artifact_path: runtime_status.last_artifact_path,
            next_action: runtime_status.next_action,
        },
        args.json,
    )
}

fn host_thread_existence_snapshot(host_thread_id: Option<&str>) -> (String, String) {
    let Some(host_thread_id) = host_thread_id else {
        return (
            "not-applicable".to_string(),
            "no hostThreadId is available for this status request".to_string(),
        );
    };
    let current_thread_id = std::env::var("CODEX_THREAD_ID").ok();
    if current_thread_id.as_deref() == Some(host_thread_id) {
        return (
            "current-thread-active".to_string(),
            "hostThreadId matches the current CODEX_THREAD_ID; this proves the root Codex thread is present".to_string(),
        );
    }
    (
        "not-validated".to_string(),
        "ASP lifecycle does not use non-structural Codex thread listing as a control-plane source; host presence is not proven unless current CODEX_THREAD_ID, rollout ledger, and registry identity agree".to_string(),
    )
}

fn multi_agent_child_state_snapshot(
    rollout_activity: Option<
        &super::agent_session_registry_rollout_activity::RolloutActivityReport,
    >,
) -> String {
    match rollout_activity {
        Some(activity) if activity.running_session_closed => {
            "control-plane-running-session-closed".to_string()
        }
        Some(_) => "control-plane-running-session-open-or-unknown".to_string(),
        None => "not-reported".to_string(),
    }
}

fn session_lifecycle_index(
    root_session_id: Option<&str>,
    selected_name: Option<&str>,
    record: Option<&AgentSessionRecord>,
    registry_status: &str,
    routable: bool,
    rollout_session_index: Option<&CodexRolloutSessionIndex>,
    rollout_index_error: Option<&str>,
) -> SessionLifecycleIndex {
    let rollout_status_by_session = rollout_session_index
        .map(|index| {
            index
                .activity_by_session
                .iter()
                .map(|(session_id, activity)| (session_id.clone(), activity.status.clone()))
                .collect()
        })
        .unwrap_or_default();
    let missing_rollout_by_session = rollout_session_index
        .map(|index| index.missing_rollout_by_session.clone())
        .unwrap_or_default();
    SessionLifecycleIndex {
        root_session_id: root_session_id.map(str::to_string),
        selected_name: selected_name.map(str::to_string),
        selected_session_id: record.map(|record| record.session_id.clone()),
        selected_role: record.map(|record| record.role.clone()),
        registry_status: registry_status.to_string(),
        routable,
        rollout_session_count: rollout_session_index
            .map(|index| index.records.len())
            .unwrap_or_default(),
        rollout_activity_count: rollout_session_index
            .map(|index| index.activity_by_session.len())
            .unwrap_or_default(),
        missing_rollout_count: missing_rollout_by_session.len(),
        rollout_index_error: rollout_index_error.map(str::to_string),
        missing_rollout_by_session,
        rollout_status_by_session,
    }
}

pub(super) fn close_session(
    registry: &AgentSessionRegistry,
    args: &SessionArgs,
) -> Result<(), String> {
    let project_id = current_project_session_scope_id()?;
    registry.refresh_expired_sessions()?;
    let record = lifecycle_target_session(registry, args, &project_id)?;
    let now = agent_session_unix_timestamp()?;
    let archived = registry.archive_session(&project_id, &record.session_id, now)?;
    if args.json {
        print_lifecycle_json(
            "close",
            &[record.session_id.clone()],
            1,
            usize::from(archived),
            Some("archived"),
        )
    } else {
        println!(
            "[agent-session-close] archived={} sessionId={} name={} role={}",
            archived, record.session_id, record.name, record.role
        );
        Ok(())
    }
}

pub(super) fn gc_sessions(
    registry: &AgentSessionRegistry,
    args: &SessionArgs,
) -> Result<(), String> {
    let project_id = current_project_session_scope_id()?;
    registry.refresh_expired_sessions()?;
    let root_session_id = resolved_root_session_id(registry, args.root_session_id.as_deref())?;
    let candidates = if args.child_session_id.is_some() {
        lifecycle_target_session(registry, args, &project_id).map(|record| vec![record])?
    } else {
        registry.query_sessions(
            &project_id,
            root_session_id.as_deref(),
            args.name.as_deref(),
        )?
    };
    let mut inspected = 0usize;
    let mut deleted = Vec::new();
    for record in candidates {
        inspected += 1;
        if args.force || is_gc_candidate_status(&record.status) {
            if registry.delete_session(&project_id, &record.session_id)? {
                deleted.push(record.session_id);
            }
        }
    }
    if args.json {
        print_lifecycle_json("gc", &deleted, inspected, deleted.len(), None)
    } else {
        println!(
            "[agent-session-gc] inspected={} deleted={}",
            inspected,
            deleted.len()
        );
        for session_id in deleted {
            println!("{session_id}");
        }
        Ok(())
    }
}

pub(super) fn reconcile_sessions(
    registry: &AgentSessionRegistry,
    args: &SessionArgs,
) -> Result<(), String> {
    let project_id = current_project_session_scope_id()?;
    registry.refresh_expired_sessions()?;
    let root_session_id = resolved_root_session_id(registry, args.root_session_id.as_deref())?;
    let sessions = registry.query_sessions(
        &project_id,
        root_session_id.as_deref(),
        args.name.as_deref(),
    )?;
    let gc_candidates = sessions
        .iter()
        .filter(|record| is_gc_candidate_status(&record.status))
        .count();
    if args.json {
        print_lifecycle_json(
            "reconcile",
            &[],
            sessions.len(),
            gc_candidates,
            Some("refreshed-expired-sessions"),
        )
    } else {
        println!(
            "[agent-session-reconcile] refreshed=true sessions={} gcCandidates={}",
            sessions.len(),
            gc_candidates
        );
        Ok(())
    }
}

fn lifecycle_target_session(
    registry: &AgentSessionRegistry,
    args: &SessionArgs,
    project_id: &str,
) -> Result<AgentSessionRecord, String> {
    if args.child_session_id.is_none() && args.name.is_none() {
        return Err("session lifecycle command requires --child-session-id or --name".to_string());
    }
    let root_session_id = resolved_root_session_id(registry, args.root_session_id.as_deref())?;
    registry
        .lookup_session(AgentSessionLookupRequest {
            project_id,
            session_id: args.child_session_id.as_deref(),
            root_session_id: root_session_id.as_deref(),
            name: args.name.as_deref(),
        })?
        .ok_or_else(|| "session lifecycle target not found".to_string())
}

fn is_gc_candidate_status(status: &str) -> bool {
    matches!(
        status,
        "archived" | "closed" | "expired" | AGENT_SESSION_STATUS_INVALID
    )
}

fn print_lifecycle_json(
    command: &str,
    session_ids: &[String],
    inspected: usize,
    affected: usize,
    status: Option<&str>,
) -> Result<(), String> {
    let report = serde_json::json!({
        "owner": "rust",
        "command": command,
        "inspected": inspected,
        "affected": affected,
        "status": status,
        "sessionIds": session_ids,
    });
    println!(
        "{}",
        serde_json::to_string_pretty(&report)
            .map_err(|error| format!("failed to render lifecycle json: {error}"))?
    );
    Ok(())
}

fn activity_snapshot_short(
    root_session_id: Option<&str>,
    record: Option<&AgentSessionRecord>,
    registry_status: &str,
    host_status: &str,
    health_status: &str,
    next_action: &str,
    rollout_activity: Option<
        &super::agent_session_registry_rollout_activity::RolloutActivityReport,
    >,
    rollout_session_index: Option<&CodexRolloutSessionIndex>,
    rollout_index_error: Option<&str>,
) -> ActivitySnapshotShort {
    let rollout_status_by_session = rollout_session_index
        .map(|index| {
            index
                .activity_by_session
                .iter()
                .map(|(session_id, activity)| (session_id.clone(), activity.status.clone()))
                .collect()
        })
        .unwrap_or_default();
    let missing_rollout_by_session = rollout_session_index
        .map(|index| index.missing_rollout_by_session.clone())
        .unwrap_or_default();
    ActivitySnapshotShort {
        source: "codex-rollout-session-index",
        root_session_id: root_session_id.map(str::to_string),
        selected_session_id: record.map(|record| record.session_id.clone()),
        selected_role: record.map(|record| record.role.clone()),
        registry_status: registry_status.to_string(),
        host_status: host_status.to_string(),
        health_status: health_status.to_string(),
        next_action: next_action.to_string(),
        rollout_activity_status: rollout_activity.map(|activity| activity.status.clone()),
        rollout_last_heartbeat_kind: rollout_activity
            .and_then(|activity| activity.last_heartbeat_kind.clone()),
        rollout_last_terminal_event: rollout_activity
            .and_then(|activity| activity.last_terminal_event.clone()),
        rollout_running_session_closed: rollout_activity
            .map(|activity| activity.running_session_closed),
        seconds_since_heartbeat: rollout_activity
            .and_then(|activity| activity.seconds_since_heartbeat),
        rollout_index_error: rollout_index_error.map(str::to_string),
        missing_rollout_by_session,
        rollout_status_by_session,
    }
}
