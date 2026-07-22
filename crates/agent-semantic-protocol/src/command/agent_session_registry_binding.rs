use agent_semantic_client_db::agent_session_registry::{
    AgentSessionModelObservationRef, AgentSessionModelObservationSource, AgentSessionRecord,
    AgentSessionRegisterRequest, AgentSessionRegistry,
};

use super::reasoning::{
    rollout_proves_canonical_typed_binding, typed_subagent_start_proves_canonical_typed_binding,
};

pub(in crate::command) fn invalidate_unroutable_canonical_target(
    registry: &AgentSessionRegistry,
    project_id: &str,
    existing: Option<&mut AgentSessionRecord>,
    host_target_absent: bool,
    now: i64,
) -> Result<bool, String> {
    let Some(existing) = existing.filter(|existing| {
        host_target_absent && !matches!(existing.status.as_str(), "archived" | "closed")
    }) else {
        return Ok(false);
    };
    *existing = registry
        .invalidate_session_live_binding(project_id, &existing.session_id, "orphan-risk", now)?
        .ok_or_else(|| {
            format!(
                "failed to invalidate absent resident child `{}`",
                existing.session_id
            )
        })?;
    Ok(true)
}

fn bind_verified_canonical_target(
    registry: &AgentSessionRegistry,
    project_id: &str,
    root_session_id: &str,
    existing: &AgentSessionRecord,
    name: &str,
    message_target_id: &str,
    binding_source: &str,
    now: i64,
) -> Result<AgentSessionRecord, Box<dyn std::error::Error>> {
    let mut metadata = serde_json::from_str::<serde_json::Value>(&existing.metadata_json)
        .unwrap_or_else(|_| serde_json::json!({}));
    if !metadata.is_object() {
        metadata = serde_json::json!({});
    }
    metadata["messageTargetBinding"] = serde_json::json!({
        "source": binding_source,
        "boundRootSessionId": root_session_id,
        "childSessionId": existing.session_id,
        "messageTargetId": message_target_id,
        "observedAt": now,
    });
    let model_observation = match (
        existing.model.as_deref(),
        existing.model_observation_source.as_deref(),
        existing.model_observed_at,
    ) {
        (Some(model), Some("codex.subagent-start"), Some(observed_at)) => {
            Some(AgentSessionModelObservationRef {
                model,
                source: AgentSessionModelObservationSource::CodexSubagentStart,
                observed_at,
                evidence_ref: existing.model_evidence_ref.as_deref(),
            })
        }
        (Some(model), Some("codex.rollout"), Some(observed_at)) => {
            Some(AgentSessionModelObservationRef {
                model,
                source: AgentSessionModelObservationSource::CodexRollout,
                observed_at,
                evidence_ref: existing.model_evidence_ref.as_deref(),
            })
        }
        _ => None,
    };
    registry.archive_session(project_id, &existing.session_id, now)?;
    Ok(
        registry.claim_resident_session(AgentSessionRegisterRequest {
            project_id,
            root_session_id,
            session_id: &existing.session_id,
            message_target_id: Some(message_target_id),
            parent_session_id: Some(root_session_id),
            name,
            role: &existing.role,
            model_observation,
            status: "idle",
            expires_at: None,
            metadata_json: &metadata.to_string(),
            now,
        })?,
    )
}

pub(super) fn require_host_tree_audit<'a>(
    menu: agent_semantic_client_db::agent_session_registry::AgentSessionInteractiveMenu<'a>,
    observation_missing: bool,
) -> agent_semantic_client_db::agent_session_registry::AgentSessionInteractiveMenu<'a> {
    if observation_missing {
        agent_semantic_client_db::agent_session_registry::resident_child_host_tree_audit_required_menu(menu)
    } else {
        menu
    }
}

pub(super) fn insert_non_present_canonical_target_receipt(
    object: &mut serde_json::Map<String, serde_json::Value>,
    record: Option<&AgentSessionRecord>,
    target_status: &str,
    typed_spawn_status: Option<&str>,
    canonical_target: &str,
) {
    let Some(record) = record else {
        return;
    };
    if target_status == "absent" {
        object.insert("bootstrapBlocked".to_string(), serde_json::Value::Null);
        object.insert(
            "canonicalBindingObservation".to_string(),
            serde_json::json!({
                "status": "host-tree-absent-reachability-unprobed",
                "childSessionId": record.session_id,
                "canonicalTarget": canonical_target,
                "messageTargetStatus": "probe-required",
                "registryRoutable": false,
                "reasoningGateEvaluated": false,
                "reasoningVerificationStatus": serde_json::Value::Null,
                "reasoningEvidenceSource": serde_json::Value::Null,
                "nextAction": "probe-hidden-routable-child-before-replacement",
            }),
        );
        return;
    }
    if target_status != "unroutable" {
        return;
    }
    let (next_action, blocker) = match typed_spawn_status {
        Some("present") => (
            "create-canonical-typed-child-after-orphaned-owner",
            serde_json::Value::Null,
        ),
        Some("absent") => (
            "blocked-host-typed-spawn-unavailable",
            serde_json::Value::String("host-typed-spawn-unavailable".to_string()),
        ),
        _ => (
            "audit-host-typed-spawn-schema",
            serde_json::Value::String("host-typed-spawn-audit-required".to_string()),
        ),
    };
    object.insert("bootstrapBlocked".to_string(), blocker);
    object.insert(
        "canonicalBindingObservation".to_string(),
        serde_json::json!({
            "status": "historical-only-non-rebindable",
            "childSessionId": record.session_id,
            "canonicalTarget": canonical_target,
            "messageTargetStatus": "unbound",
            "registryRoutable": false,
            "reasoningGateEvaluated": false,
            "reasoningVerificationStatus": serde_json::Value::Null,
            "reasoningEvidenceSource": serde_json::Value::Null,
            "nextAction": next_action,
        }),
    );
}

pub(in crate::command) fn maybe_bind_verified_canonical_target(
    registry: &AgentSessionRegistry,
    existing: Option<&AgentSessionRecord>,
    host_target_present: bool,
    canonical_target: Option<&str>,
    expected_agent_type: &str,
    expected_model: Option<&str>,
    host_observed_model: Option<&str>,
) -> Result<Option<AgentSessionRecord>, String> {
    let (Some(existing), Some(canonical_target), Some(expected_model)) =
        (existing, canonical_target, expected_model)
    else {
        return Ok(None);
    };
    let root_session_id = existing.root_session_id.as_str();
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|error| error.to_string())?
        .as_secs() as i64;
    let model_matches = host_observed_model == Some(expected_model)
        || existing.model.as_deref() == Some(expected_model);
    if !host_target_present
        || !model_matches
        || agent_semantic_client_db::agent_session_registry::agent_session_message_target_is_live_bound(
            existing,
            root_session_id,
        )
    {
        return Ok(None);
    }
    let binding_source = if typed_subagent_start_proves_canonical_typed_binding(
        existing,
        root_session_id,
        expected_agent_type,
    ) {
        "codex-typed-subagent-start-plus-native-host-tree"
    } else if rollout_proves_canonical_typed_binding(
        &existing.session_id,
        root_session_id,
        expected_agent_type,
        canonical_target,
    ) {
        "codex-rollout-session-meta-plus-native-host-tree"
    } else if canonical_target == format!("/root/{expected_agent_type}") {
        "codex-locked-generation-profile-plus-native-host-tree"
    } else {
        return Ok(None);
    };
    bind_verified_canonical_target(
        registry,
        &existing.project_id,
        root_session_id,
        existing,
        &existing.name,
        canonical_target,
        binding_source,
        now,
    )
    .map(Some)
    .map_err(|error| error.to_string())
}

#[cfg(test)]
#[path = "../../tests/unit/agent_session_registry_binding.rs"]
mod agent_session_registry_binding_tests;
