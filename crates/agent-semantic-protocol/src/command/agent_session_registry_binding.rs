use agent_semantic_client_db::agent_session_registry::{
    AgentSessionModelObservationRef, AgentSessionModelObservationSource, AgentSessionRecord,
    AgentSessionRegisterRequest, AgentSessionRegistry,
};

use super::reasoning::{
    rollout_proves_canonical_typed_binding, typed_subagent_start_proves_canonical_typed_binding,
};

fn bind_verified_canonical_target(
    registry: &AgentSessionRegistry,
    project_id: &str,
    root_session_id: &str,
    existing: &AgentSessionRecord,
    name: &str,
    binding_source: &str,
    now: i64,
) -> Result<AgentSessionRecord, Box<dyn std::error::Error>> {
    let message_target_id = "/root/asp_explorer";
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

pub(super) fn insert_absent_canonical_target_receipt(
    object: &mut serde_json::Map<String, serde_json::Value>,
    record: Option<&AgentSessionRecord>,
    typed_spawn_status: Option<&str>,
) {
    let Some(record) = record else {
        return;
    };
    let (next_action, blocker) = match typed_spawn_status {
        Some("present") => (
            "create-canonical-typed-child-after-orphaned-owner",
            serde_json::Value::Null,
        ),
        Some("absent") => (
            "activate-inline-parser-fallback",
            serde_json::Value::String("host-agent-type-unavailable".to_string()),
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
            "canonicalTarget": "/root/asp_explorer",
            "messageTargetStatus": "unbound",
            "registryRoutable": false,
            "reasoningGateEvaluated": false,
            "reasoningVerificationStatus": serde_json::Value::Null,
            "reasoningEvidenceSource": serde_json::Value::Null,
            "nextAction": next_action,
        }),
    );
}

pub(super) fn maybe_bind_verified_canonical_target(
    registry: &AgentSessionRegistry,
    existing: Option<&AgentSessionRecord>,
    host_target_present: bool,
    expected_model: Option<&str>,
    host_observed_model: Option<&str>,
) -> Result<Option<AgentSessionRecord>, String> {
    let (Some(existing), Some(expected_model)) = (existing, expected_model) else {
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
    let binding_source =
        if typed_subagent_start_proves_canonical_typed_binding(existing, root_session_id) {
            "codex-typed-subagent-start-plus-native-host-tree"
        } else if rollout_proves_canonical_typed_binding(&existing.session_id, root_session_id) {
            "codex-rollout-session-meta-plus-native-host-tree"
        } else {
            return Ok(None);
        };
    bind_verified_canonical_target(
        registry,
        &existing.project_id,
        root_session_id,
        existing,
        &existing.name,
        binding_source,
        now,
    )
    .map(Some)
    .map_err(|error| error.to_string())
}
