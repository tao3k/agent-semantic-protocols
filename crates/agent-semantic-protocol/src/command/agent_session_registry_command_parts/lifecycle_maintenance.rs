use agent_semantic_client_db::{
    AGENT_SESSION_STATUS_INVALID, AgentSessionLookupRequest, AgentSessionRecord,
    AgentSessionRegisterRequest, AgentSessionRegistry, agent_session_message_target_is_live_bound,
    agent_session_unix_timestamp,
};
use agent_semantic_runtime::{
    agent_session_registration_identity, agent_session_runtime_status_snapshot,
};

use crate::command::agent_session_registry::agent_session_registry_args::SessionArgs;
use crate::command::agent_session_registry::agent_session_registry_lifetime::resolve_session_lifetime;
use crate::command::agent_session_registry::agent_session_registry_render::{
    SessionStatusReport, escape_field, print_json_report, print_session_row, print_status_report,
};
use crate::command::agent_session_registry::agent_session_registry_rollout_activity::rollout_activity_report;
use crate::command::agent_session_registry::agent_session_registry_rollout_adopt::{
    RolloutAdoptRequest, adopt_reusable_rollout_session,
};
use crate::command::agent_session_registry::agent_session_registry_state::{
    current_project_session_scope_id, current_recall_session_id, project_session_scope_id,
    required_non_empty, resolved_root_session_id, session_record_validation_allows_routing,
};
use crate::command::agent_session_registry::agent_session_registry_validation::{
    validate_recent_session_profile, validate_session_profile,
};
use crate::command::agent_session_registry::normalized_metadata_with_roles;
use crate::command::agent_session_registry::stale_invalid_session_should_be_idle;
use std::path::Path;

pub(in crate::command::agent_session_registry) fn close_session(
    registry: &AgentSessionRegistry,
    args: &SessionArgs,
) -> Result<(), String> {
    let project_id = current_project_session_scope_id(registry)?;
    registry.refresh_expired_sessions()?;
    let record = lifecycle_target_session(registry, args, &project_id)?;
    let now = agent_session_unix_timestamp()?;
    let archived = registry.archive_session(&project_id, &record.session_id, now)?;
    if args.json {
        print_lifecycle_json(
            "close",
            std::slice::from_ref(&record.session_id),
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

pub(in crate::command::agent_session_registry) fn gc_sessions(
    registry: &AgentSessionRegistry,
    args: &SessionArgs,
) -> Result<(), String> {
    let project_id = current_project_session_scope_id(registry)?;
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
    let inspected = candidates.len();
    let deleted = candidates
        .into_iter()
        .map(|record| {
            let eligible = args.force || is_gc_candidate_status(&record.status);
            let deleted = eligible && registry.delete_session(&project_id, &record.session_id)?;
            Ok(deleted.then_some(record.session_id))
        })
        .collect::<Result<Vec<_>, String>>()?
        .into_iter()
        .flatten()
        .collect::<Vec<_>>();
    if args.json {
        print_lifecycle_json("gc", &deleted, inspected, deleted.len(), None)
    } else {
        println!(
            "[agent-session-gc] inspected={} deleted={}",
            inspected,
            deleted.len()
        );
        deleted
            .into_iter()
            .for_each(|session_id| println!("{session_id}"));
        Ok(())
    }
}

pub(in crate::command::agent_session_registry) fn reconcile_sessions(
    registry: &AgentSessionRegistry,
    args: &SessionArgs,
) -> Result<(), String> {
    let project_id = current_project_session_scope_id(registry)?;
    registry.refresh_expired_sessions()?;
    let now = agent_session_unix_timestamp()?;
    let root_session_id = resolved_root_session_id(registry, args.root_session_id.as_deref())?;
    let sessions = registry.query_sessions(
        &project_id,
        root_session_id.as_deref(),
        args.name.as_deref(),
    )?;
    let reconciled_session_ids = sessions
        .iter()
        .map(|record| {
            let stale = stale_invalid_session_should_be_idle(record, now)?;
            let reconciled = stale
                && registry.update_session_status(&project_id, &record.session_id, "idle", now)?;
            Ok(reconciled.then(|| record.session_id.clone()))
        })
        .collect::<Result<Vec<_>, String>>()?
        .into_iter()
        .flatten()
        .collect::<Vec<_>>();
    let gc_candidates = sessions
        .iter()
        .filter(|record| is_gc_candidate_status(&record.status))
        .count();
    if args.json {
        print_lifecycle_json(
            "reconcile",
            &reconciled_session_ids,
            sessions.len(),
            gc_candidates,
            Some("refreshed-expired-and-reconciled-rollout-sessions"),
        )
    } else {
        println!(
            "[agent-session-reconcile] refreshed=true reconciled={} sessions={} gcCandidates={}",
            reconciled_session_ids.len(),
            sessions.len(),
            gc_candidates
        );
        Ok(())
    }
}

pub(super) fn lifecycle_target_session(
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

pub(super) fn is_gc_candidate_status(status: &str) -> bool {
    matches!(
        status,
        "archived" | "closed" | "expired" | AGENT_SESSION_STATUS_INVALID
    )
}

pub(super) fn print_lifecycle_json(
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
