//! Registry ownership transition for a host-created typed resident replacement.

use std::path::Path;

pub(super) fn session_start_decision_for_reconciled_resident(
    now: i64,
    platform: &str,
    event: &str,
    payload: &serde_json::Value,
    existing: &crate::codex::resident_session_reconcile::CodexResidentSessionCandidate,
    asp_session_policy: &AspSessionPolicy,
) -> super::HookDecision {
    use super::hook_runtime_agent_session_session_start::{
        session_start_resume_existing_decision, session_start_reuse_decision,
    };
    use crate::codex::rollout::CodexRolloutSessionLiveness;

    if !existing.session.is_routable_at(now) {
        return session_start_resume_existing_decision(
            platform,
            event,
            payload,
            &existing.session,
            asp_session_policy,
        );
    }
    match &existing.liveness {
        CodexRolloutSessionLiveness::Active(_) | CodexRolloutSessionLiveness::Unknown(_) => {
            session_start_reuse_decision(
                platform,
                event,
                payload,
                &existing.session,
                asp_session_policy,
            )
        }
        CodexRolloutSessionLiveness::Resumable(_)
        | CodexRolloutSessionLiveness::Missing
        | CodexRolloutSessionLiveness::Unavailable(_) => session_start_resume_existing_decision(
            platform,
            event,
            payload,
            &existing.session,
            asp_session_policy,
        ),
    }
}

pub(super) fn append_resident_reconciliation_fields(
    decision: &mut super::HookDecision,
    reconciliation: &crate::codex::resident_session_reconcile::CodexResidentSessionReconciliation,
) {
    use crate::codex::rollout::CodexRolloutSessionLiveness;

    let Some(current) = reconciliation.current.as_ref() else {
        return;
    };
    let (state, activity, error) = match &current.liveness {
        CodexRolloutSessionLiveness::Resumable(activity) => {
            ("rollout-resumable", Some(activity), None)
        }
        CodexRolloutSessionLiveness::Active(activity) => ("rollout-active", Some(activity), None),
        CodexRolloutSessionLiveness::Unknown(activity) => ("rollout-unknown", Some(activity), None),
        CodexRolloutSessionLiveness::Missing => ("rollout-missing", None, None),
        CodexRolloutSessionLiveness::Unavailable(error) => {
            ("rollout-unavailable", None, Some(error.as_str()))
        }
    };
    decision.fields.insert(
        "agentSessionReconciliation".to_string(),
        serde_json::json!(state),
    );
    decision.fields.insert(
        "agentSessionRolloutLookup".to_string(),
        serde_json::json!("session-id-fast-path"),
    );
    decision.fields.insert(
        "agentSessionHistoricalResidentCount".to_string(),
        serde_json::json!(reconciliation.historical_resident_count),
    );
    if let Some(activity) = activity {
        if let Some(kind) = activity.last_event_kind.as_ref() {
            decision.fields.insert(
                "agentSessionRolloutLastEventKind".to_string(),
                serde_json::json!(kind),
            );
        }
        decision.fields.insert(
            "agentSessionRolloutScannedBytes".to_string(),
            serde_json::json!(activity.scanned_bytes),
        );
    }
    if let Some(error) = error {
        decision.fields.insert(
            "agentSessionRolloutError".to_string(),
            serde_json::json!(error),
        );
    }
}

use super::AspSessionPolicy;

pub(super) fn release_terminal_owner_before_typed_start(
    project_root: &Path,
    native: &crate::codex::native_agent_transport::CodexNativeSubagentEvent,
    root_session_id: &str,
    asp_session_policy: &AspSessionPolicy,
) -> Result<(), String> {
    let registry =
        agent_semantic_client_db::AgentSessionRegistry::open_or_create_project(project_root)?;
    let project_id = agent_semantic_client_db::AgentSessionRegistry::project_scope_id(project_root);
    let replacement_lease =
        crate::command::agent_session_registry::agent_session_registry_host_capability::consume_fresh_absent_resident_target_observation(
            &registry,
            root_session_id,
            asp_session_policy.resident_child_name(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_err(|error| format!("read resident replacement timestamp: {error}"))?
                .as_secs() as i64,
        )?;
    let Some(existing) = registry
        .query_sessions(
            &project_id,
            Some(root_session_id),
            Some(asp_session_policy.resident_child_name()),
        )?
        .into_iter()
        .find(|existing| {
            existing.session_id != native.agent_id
                && (replacement_lease
                    || matches!(
                        existing.status.as_str(),
                        "invalid" | "replacement-required" | "orphan-risk" | "archived" | "closed"
                    ))
        })
    else {
        return Ok(());
    };
    registry
        .delete_session(&project_id, &existing.session_id)
        .map(|_| ())
}
