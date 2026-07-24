use std::path::Path;

pub(super) fn rehydrate_trusted_resident_hook_session(
    project_root: &Path,
    root_session_id: &str,
    child_session_id: &str,
    resident_child_name: &str,
    resident_agent_role: &str,
    resident_codex_agent_name: &str,
) -> Result<Option<agent_semantic_client_db::AgentSessionRecord>, String> {
    let Some(rollout) = agent_semantic_runtime::codex_rollout_session_metadata(child_session_id)?
    else {
        return Ok(None);
    };
    if child_session_id == root_session_id
        || rollout.root_session_id.as_deref() != Some(root_session_id)
        || !crate::command::rollout_metadata_matches_managed_agent_profile(
            resident_child_name,
            resident_agent_role,
            &rollout,
        )
    {
        return Ok(None);
    }
    let validation = crate::command::validate_session_profile(
        child_session_id,
        root_session_id,
        resident_child_name,
        resident_agent_role,
        agent_semantic_client_db::agent_session_unix_timestamp()?,
    )?;
    if validation.status == "failed"
        || validation.actual_model != validation.expected_model
        || validation.actual_reasoning_effort.is_some()
            && validation.actual_reasoning_effort != validation.expected_reasoning_effort
    {
        return Ok(None);
    }
    let Some(model) = rollout.model.as_deref() else {
        return Ok(None);
    };
    let registry =
        agent_semantic_client_db::AgentSessionRegistry::open_or_create_project(project_root)?;
    let project_id = agent_semantic_client_db::AgentSessionRegistry::project_scope_id(project_root);
    let Some(existing) =
        registry.session_by_name(&project_id, root_session_id, resident_child_name)?
    else {
        return Ok(None);
    };
    if existing.session_id == child_session_id {
        return Ok(Some(existing));
    }
    let now = agent_semantic_client_db::agent_session_unix_timestamp()?;
    let canonical_target = format!("/root/{resident_codex_agent_name}");
    let metadata = serde_json::json!({
        "messageTargetBinding": {
            "source": "codex-hook-payload-plus-rollout-profile",
            "boundRootSessionId": root_session_id,
            "childSessionId": child_session_id,
            "messageTargetId": canonical_target,
            "boundAt": now,
        },
        "identityRehydration": {
            "source": "exact-hook-child-plus-rollout-profile",
            "previousChildSessionId": existing.session_id,
            "childSessionId": child_session_id,
        }
    });
    let metadata_json = agent_semantic_client_db::agent_session_normalized_metadata_json(
        Some(&metadata.to_string()),
        &validation,
    )?;
    let model_observation = agent_semantic_client_db::AgentSessionModelObservationRef {
        model,
        source: agent_semantic_client_db::AgentSessionModelObservationSource::CodexRollout,
        observed_at: now,
        evidence_ref: Some(child_session_id),
    };
    registry
        .replace_resident_session(
            &existing.session_id,
            agent_semantic_client_db::AgentSessionRegisterRequest {
                project_id: &project_id,
                root_session_id,
                session_id: child_session_id,
                message_target_id: Some(&canonical_target),
                parent_session_id: Some(root_session_id),
                name: resident_child_name,
                role: resident_agent_role,
                model_observation: Some(model_observation),
                status: agent_semantic_client_db::AGENT_SESSION_STATUS_ACTIVE,
                expires_at: existing.expires_at,
                metadata_json: &metadata_json,
                now,
            },
        )
        .map(Some)
}

pub(super) fn record_trusted_resident_hook_presence(
    project_root: &Path,
    session: &agent_semantic_client_db::AgentSessionRecord,
    resident_child_name: &str,
    resident_codex_agent_name: &str,
) -> Result<(), String> {
    let Some(registry) =
        agent_semantic_client_db::AgentSessionRegistry::open_existing_project(project_root)?
    else {
        return Ok(());
    };
    let observed_at = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|error| error.to_string())?
        .as_secs() as i64;
    crate::command::agent_session_registry::agent_session_registry_host_capability::record_trusted_resident_hook_target_present(
        &registry,
        crate::command::agent_session_registry::agent_session_registry_host_capability::TrustedResidentHookTargetPresentInput {
            project_id: &session.project_id,
            root_session_id: &session.root_session_id,
            resident_name: resident_child_name,
            canonical_target: &format!("/root/{resident_codex_agent_name}"),
            observed_at,
        },
    )
}
