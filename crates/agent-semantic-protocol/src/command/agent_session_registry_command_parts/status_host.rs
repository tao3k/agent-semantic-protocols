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
use std::path::Path;

pub(super) fn status_session(
    registry: &AgentSessionRegistry,
    args: &SessionArgs,
    project_root: &Path,
) -> Result<(), String> {
    if args.activity
        && let Some(child_session_id) = args.child_session_id.as_deref()
    {
        let now = agent_session_unix_timestamp()?;
        if let Some(rollout_path) =
            crate::command::agent_session_registry::agent_session_registry_rollout_lookup::fast_rollout_path_for_session_id(
                child_session_id,
            )
        {
            let report = rollout_activity_report(&rollout_path, now);
            return crate::command::agent_session_registry::agent_session_registry_render::print_status_activity_report(
                Some(&report),
                report.agent_instruction.as_str(),
                args.json,
            );
        }
    }

    let project_id = project_session_scope_id(registry, project_root)?;
    let root_session_id = resolved_root_session_id(registry, args.root_session_id.as_deref())?;
    let name = args.name.clone();
    let mut record = registry.lookup_session(AgentSessionLookupRequest {
        project_id: &project_id,
        session_id: args.child_session_id.as_deref(),
        root_session_id: root_session_id.as_deref(),
        name: name.as_deref(),
    })?;
    let now = agent_session_unix_timestamp()?;
    let host_resident_target_observation = root_session_id
        .as_deref()
        .zip(name.as_deref())
        .map(|(root_session_id, name)| {
            crate::command::agent_session_registry::agent_session_registry_host_capability::fresh_host_resident_target_observation(
                    registry,
                    root_session_id,
                    name,
                    now,
                )
        })
        .transpose()?
        .flatten();
    let fresh_host_transport_verified =
        host_resident_target_observation
            .as_ref()
            .is_some_and(|observation| {
                observation.target_status == "present" && observation.identity_status == "verified"
            });
    let host_target_absent = host_resident_target_observation
        .as_ref()
        .is_some_and(|observation| observation.target_status == "absent");
    if host_target_absent && let Some(session) = record.as_mut() {
        session.status = "orphan-risk".to_string();
    }
    if let Some(session) = record.as_mut()
        && stale_invalid_session_should_be_idle(session, now)?
    {
        session.status = "idle".to_string();
    }
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
    if let (Some(session), Some(validation)) = (record.as_mut(), validation.as_ref())
        && validation.status == "failed"
    {
        session.status = "invalid".to_string();
    }
    let registry_allows_routing = record
        .as_ref()
        .map(|session| {
            !matches!(session.status.as_str(), "archived" | "closed")
                && agent_semantic_client_db::agent_session_message_target_is_currently_routable(
                    session,
                    &session.root_session_id,
                    fresh_host_transport_verified,
                    now,
                )
        })
        .unwrap_or(false);
    let validation_allows_routing = validation.as_ref().is_none_or(|validation| {
        matches!(validation.status.as_str(), "passed" | "warning" | "skipped")
    });
    let required_model = validation
        .as_ref()
        .and_then(|validation| validation.expected_model.clone());
    let actual_model = validation
        .as_ref()
        .and_then(|validation| validation.actual_model.clone());
    let rollout_activity = validation
        .as_ref()
        .and_then(|validation| validation.rollout_path.as_deref())
        .map(|rollout_path| rollout_activity_report(Path::new(rollout_path), now));
    if args.activity {
        let next_action = rollout_activity
            .as_ref()
            .map(|activity| activity.agent_instruction.as_str())
            .unwrap_or("register-existing-child-or-replace-only-after-host-confirms-unrecoverable");
        return crate::command::agent_session_registry::agent_session_registry_render::print_status_activity_report(
            rollout_activity.as_ref(),
            next_action,
            args.json,
        );
    }
    let lifecycle_allows_routing = rollout_activity
        .as_ref()
        .map(|activity| !activity.running_session_closed)
        .unwrap_or(true);
    let mut routable =
        registry_allows_routing && validation_allows_routing && lifecycle_allows_routing;
    let registry_status = record
        .as_ref()
        .map(|session| session.status.clone())
        .unwrap_or_else(|| "missing".to_string());
    let host_thread_id = record
        .as_ref()
        .map(|session| session.session_id.as_str())
        .or(root_session_id.as_deref());
    let runtime_status = agent_session_runtime_status_snapshot(
        (
            project_root,
            now,
            args.artifact_stale_after_seconds,
            host_thread_id,
            record.is_some(),
            registry_allows_routing,
        )
            .into(),
    )?;
    let rollout_session_index = None;
    if routable
        && rollout_activity
            .as_ref()
            .and_then(|activity| activity.session_activity.as_ref())
            .is_some_and(|activity| {
                matches!(
                    activity.status.as_str(),
                    "tool-running" | "agent-active" | "idle-resumable"
                )
            })
    {
        routable = true;
    }
    let session_lifecycle_index = None;
    let activity_snapshot_short = None;
    let (host_thread_existence, host_thread_existence_reason) =
        host_thread_existence_snapshot(runtime_status.host_thread_id.as_deref());
    let multi_agent_child_state = multi_agent_child_state_snapshot(
        record.as_ref().map(|session| session.status.as_str()),
        routable,
        rollout_activity.as_ref(),
    );
    let session_lifetime = resolve_session_lifetime(
        project_root,
        name.as_deref(),
        runtime_status.host_client.as_deref(),
    );
    let mut report = SessionStatusReport {
        owner: "rust",
        db_path: registry.db_path().display().to_string(),
        root_session_id,
        name,
        session: record,
        registry_status,
        routable,
        session_lifetime: session_lifetime.value,
        resident: session_lifetime.resident,
        session_lifetime_source: session_lifetime.source,
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
        message_target_status: None,
        message_target_result_source: None,
        message_agent_target_id: None,
        message_agent_target_id_equals_child: None,
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
        required_model,
        actual_model,
        model_alignment_action: None,
        model_alignment_message: None,
    };
    if let Some(activity) = report.rollout_activity.as_ref() {
        report.next_action = activity.agent_instruction.clone();
    }
    match (
        report.required_model.as_deref(),
        report.actual_model.as_deref(),
    ) {
        (Some(required_model), None) => {
            report.model_alignment_action =
                Some("resume-existing-child-for-runtime-observation".to_string());
            report.model_alignment_message = Some(format!(
                "Resume the same canonical resident child once to obtain a fresh SubagentStart model observation for required model {required_model}; missing observation is not model drift. Then rerun bootstrap."
            ));
            report.next_action =
                "resume-existing-child-for-runtime-observation-then-bootstrap".to_string();
        }
        (Some(required_model), Some(actual_model)) if required_model != actual_model => {
            report.model_alignment_action =
                Some("reenter-bootstrap-for-capability-gated-typed-replacement".to_string());
            report.model_alignment_message = Some(format!(
                "Observed model {actual_model} does not match required model {required_model}. Do not ask the child to switch itself and do not create a generic replacement. Re-enter bootstrap so the live host tree and typed-spawn capability can authorize exactly one registered-profile replacement."
            ));
            report.next_action =
                "reenter-bootstrap-for-capability-gated-typed-replacement".to_string();
        }
        _ => {}
    }
    if report.model_alignment_action.is_none() && report.session.is_some() && !report.routable {
        report.next_action =
            "reenter-bootstrap-for-host-tree-target-rebind-or-typed-replacement".to_string();
    }
    if report.session.is_none() {
        report.next_action = "run-bootstrap-for-host-tree-and-typed-spawn-audit".to_string();
    }
    if report.routable && report.health_status == "registry-not-routable" {
        report.health_status = "unknown".to_string();
        if report.rollout_activity.is_none() {
            report.next_action =
                "resume-or-send-follow-up-to-same-child-before-considering-replacement".to_string();
        }
    }
    print_status_report(report, args.json)
}

pub(super) fn registered_session_is_reusable(
    registry: &AgentSessionRegistry,
    record: &AgentSessionRecord,
    now: i64,
) -> Result<bool, String> {
    if matches!(record.status.as_str(), "archived" | "closed") {
        return Ok(false);
    }
    if !record.is_routable_at(now) {
        return Ok(false);
    }
    if !agent_session_message_target_is_live_bound(record, &record.root_session_id) {
        return Ok(false);
    }
    if !session_record_validation_allows_routing(registry, record, now)? {
        return Ok(false);
    }
    let validation = validate_session_profile(
        &record.session_id,
        &record.root_session_id,
        &record.name,
        &record.role,
        now,
    )?;
    Ok(validation
        .rollout_path
        .as_deref()
        .map(|rollout_path| {
            let activity = rollout_activity_report(Path::new(rollout_path), now);
            activity
                .session_activity
                .as_ref()
                .map(|session_activity| {
                    matches!(
                        session_activity.status.as_str(),
                        "tool-running" | "agent-active" | "idle-resumable"
                    )
                })
                .unwrap_or(!activity.running_session_closed)
        })
        .unwrap_or(true))
}

pub(super) fn host_thread_existence_snapshot(host_thread_id: Option<&str>) -> (String, String) {
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

pub(super) fn multi_agent_child_state_snapshot(
    registry_status: Option<&str>,
    routable: bool,
    rollout_activity: Option<
        &crate::command::agent_session_registry::agent_session_registry_rollout_activity::RolloutActivityReport,
    >,
) -> String {
    if !routable {
        return if registry_status == Some("orphan-risk") {
            "control-plane-orphaned-unbound".to_string()
        } else {
            "not-routable".to_string()
        };
    }
    match rollout_activity {
        Some(activity)
            if activity
                .session_activity
                .as_ref()
                .is_some_and(|session_activity| session_activity.status == "idle-resumable") =>
        {
            "control-plane-idle-resumable".to_string()
        }
        Some(activity)
            if activity
                .session_activity
                .as_ref()
                .is_some_and(|session_activity| {
                    matches!(
                        session_activity.status.as_str(),
                        "tool-running" | "agent-active"
                    )
                }) =>
        {
            "control-plane-running-session-open-or-unknown".to_string()
        }
        Some(activity) if activity.running_session_closed => {
            "control-plane-running-session-closed".to_string()
        }
        Some(_) => "control-plane-running-session-open-or-unknown".to_string(),
        None => "not-reported".to_string(),
    }
}

#[cfg(test)]
#[path = "../../../tests/unit/agent_session_registry_commands.rs"]
pub(super) mod multi_agent_child_state_tests;
