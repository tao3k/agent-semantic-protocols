use agent_semantic_client_db::{AgentSessionRecord, AgentSessionRegistry};
use agent_semantic_runtime::{CodexRolloutSessionIndex, codex_rollout_session_index_for_sessions};

use super::agent_session_registry_args::SessionArgs;
use super::agent_session_registry_render::escape_field;
use super::agent_session_registry_state::{
    current_project_session_scope_id, current_recall_session_id,
};

pub(super) fn lifecycle_audit_session(
    registry: &AgentSessionRegistry,
    args: &SessionArgs,
) -> Result<(), String> {
    let project_id = current_project_session_scope_id(registry)?;
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
    let (rollout_session_index, rollout_index_error) = match root_filter.as_deref() {
        Some(root_session_id) => {
            let session_ids = sessions.iter().map(|session| session.session_id.as_str());
            match codex_rollout_session_index_for_sessions(root_session_id, session_ids) {
                Ok(index) => (index, None),
                Err(error) => (None, Some(error)),
            }
        }
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

pub(super) fn lifecycle_audit_report(
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
    let registered_status_by_session: std::collections::BTreeMap<String, String> = sessions
        .iter()
        .map(|session| (session.session_id.clone(), session.status.clone()))
        .collect();
    let registry_sessions: Vec<serde_json::Value> = sessions
        .iter()
        .map(lifecycle_registry_session_entry)
        .collect();
    let mut rollout_session_ids = std::collections::BTreeSet::<String>::new();
    let mut registered_rollout_sessions = Vec::<serde_json::Value>::new();
    let mut rollout_only_sessions = Vec::<serde_json::Value>::new();
    let mut active_subagent_rollouts = 0_usize;
    let mut rollout_only_active_count = 0_usize;
    let mut rollout_only_completed_count = 0_usize;
    let mut rollout_only_orphan_risk_count = 0_usize;
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
            let Some((session_id, entry)) =
                lifecycle_rollout_session_entry(index, record, &registered_status_by_session)?
            else {
                continue;
            };
            rollout_session_ids.insert(session_id.clone());
            let is_subagent_rollout = entry
                .get("threadSource")
                .and_then(serde_json::Value::as_str)
                == Some("subagent");
            let rollout_status = entry
                .get("rolloutStatus")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("silent");
            if is_subagent_rollout && rollout_status == "active" {
                active_subagent_rollouts += 1;
            }
            if registered_session_ids.contains(&session_id) {
                registered_rollout_sessions.push(entry);
            } else {
                if is_subagent_rollout {
                    match rollout_status {
                        "active" => rollout_only_active_count += 1,
                        "orphan-risk" => rollout_only_orphan_risk_count += 1,
                        _ => rollout_only_completed_count += 1,
                    }
                }
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
            "activeSubagentRollouts": active_subagent_rollouts,
            "registeredRolloutSessionCount": registered_rollout_sessions.len(),
            "rolloutOnlySessionCount": rollout_only_sessions.len(),
            "rolloutOnlyActiveCount": rollout_only_active_count,
            "rolloutOnlyCompletedCount": rollout_only_completed_count,
            "rolloutOnlyOrphanRiskCount": rollout_only_orphan_risk_count,
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
    registered_status_by_session: &std::collections::BTreeMap<String, String>,
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
    let registry_status = registered_status_by_session
        .get(&session_id)
        .map(String::as_str);
    if let Some(object) = entry.as_object_mut() {
        if let Some(status) = registry_status {
            object.insert("registryStatus".to_string(), serde_json::json!(status));
        }
        if let Some(activity) = index.activity_by_session.get(&session_id) {
            let rollout_status =
                lifecycle_final_rollout_status(registry_status, activity.status.as_str());
            object.insert(
                "rolloutStatus".to_string(),
                serde_json::json!(rollout_status),
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

fn lifecycle_final_rollout_status<'a>(
    registry_status: Option<&str>,
    rollout_status: &'a str,
) -> &'a str {
    if matches!(registry_status, Some("archived" | "closed")) {
        "completed"
    } else {
        rollout_status
    }
}
