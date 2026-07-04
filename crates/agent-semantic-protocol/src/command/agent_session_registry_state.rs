use agent_semantic_client_db::{
    AgentSessionRecord, AgentSessionRegistry, agent_session_unix_timestamp,
};
use agent_semantic_runtime::{
    current_agent_runtime_root_session_id, current_agent_runtime_session,
    has_current_agent_runtime_session,
};

use super::agent_session_registry_validation::validate_session_profile;
use std::path::Path;

pub(crate) fn registered_root_session_id(
    project_root: &Path,
    session_id: &str,
) -> Result<Option<String>, String> {
    let Some(registry) = open_existing_registry(project_root)? else {
        return Ok(None);
    };
    let project_id = project_session_scope_id(project_root);
    Ok(registry
        .session_by_id(&project_id, session_id)?
        .map(|record| record.root_session_id))
}

pub(crate) fn current_registered_session(
    project_root: &Path,
) -> Result<Option<AgentSessionRecord>, String> {
    let Some(session) = current_agent_runtime_session() else {
        return Ok(None);
    };
    let Some(registry) = open_existing_registry(project_root)? else {
        return Ok(None);
    };
    let project_id = project_session_scope_id(project_root);
    let Some(record) = registry.session_by_id(&project_id, &session.id)? else {
        return Ok(None);
    };
    registry.refresh_expired_sessions()?;
    let now = agent_session_unix_timestamp()?;
    if !record.is_routable_at(now)
        || !session_record_validation_allows_routing(&registry, &record, now)?
    {
        return Ok(None);
    }
    Ok(Some(record))
}

pub(crate) fn has_current_agent_session() -> bool {
    has_current_agent_runtime_session()
}

pub(crate) fn current_root_session_id() -> Option<String> {
    current_agent_runtime_root_session_id()
}

pub(crate) fn asp_explore_session_for_current_root(
    project_root: &Path,
    session_name: &str,
) -> Result<Option<AgentSessionRecord>, String> {
    let Some(registry) = open_existing_registry(project_root)? else {
        return Ok(None);
    };
    registry.refresh_expired_sessions()?;
    let project_id = project_session_scope_id(project_root);
    let Some(root_session_id) = current_recall_session_id(&registry)? else {
        return Ok(None);
    };
    let Some(record) = registry.session_by_name(&project_id, &root_session_id, session_name)?
    else {
        return Ok(None);
    };
    let now = agent_session_unix_timestamp()?;
    if !record.is_routable_at(now)
        || !session_record_validation_allows_routing(&registry, &record, now)?
    {
        return Ok(None);
    }
    Ok(Some(record))
}

pub(crate) fn asp_explore_session_record_for_current_root(
    project_root: &Path,
    session_name: &str,
) -> Result<Option<AgentSessionRecord>, String> {
    let Some(registry) = open_existing_registry(project_root)? else {
        return Ok(None);
    };
    registry.refresh_expired_sessions()?;
    let project_id = project_session_scope_id(project_root);
    let Some(root_session_id) = current_recall_session_id(&registry)? else {
        return Ok(None);
    };
    let Some(record) = registry.session_by_name(&project_id, &root_session_id, session_name)?
    else {
        return Ok(None);
    };
    Ok(Some(record))
}

pub(super) fn open_existing_registry(
    project_root: &Path,
) -> Result<Option<AgentSessionRegistry>, String> {
    Ok(Some(AgentSessionRegistry::open_or_create_project(
        project_root,
    )?))
}

pub(super) fn open_or_create_default_registry(
    project_root: &Path,
) -> Result<AgentSessionRegistry, String> {
    AgentSessionRegistry::open_or_create_project(project_root)
}

pub(super) fn current_project_session_scope_id() -> Result<String, String> {
    AgentSessionRegistry::current_project_scope_id()
}

pub(super) fn project_session_scope_id(project_root: &Path) -> String {
    AgentSessionRegistry::project_scope_id(project_root)
}

pub(super) fn current_recall_session_id(
    registry: &AgentSessionRegistry,
) -> Result<Option<String>, String> {
    let Some(session) = current_agent_runtime_session() else {
        return Ok(None);
    };
    let project_id = current_project_session_scope_id()?;
    if let Some(record) = registry.session_by_id(&project_id, &session.id)? {
        return Ok(Some(record.root_session_id));
    }
    Ok(Some(session.recall_session_id().to_string()))
}

pub(super) fn resolved_root_session_id(
    registry: &AgentSessionRegistry,
    explicit_root_session_id: Option<&str>,
) -> Result<Option<String>, String> {
    match explicit_root_session_id {
        Some(root_session_id) => Ok(Some(root_session_id.to_string())),
        None => current_recall_session_id(registry),
    }
}

pub(super) fn session_record_validation_allows_routing(
    registry: &AgentSessionRegistry,
    record: &AgentSessionRecord,
    now: i64,
) -> Result<bool, String> {
    let validation = validate_session_profile(
        &record.session_id,
        &record.root_session_id,
        &record.name,
        &record.role,
        now,
    )?;
    if validation.status == "failed" {
        if validation
            .reason
            .starts_with("Codex rollout metadata not found")
            && stored_session_validation_allows_routing(record)
        {
            return Ok(true);
        }
        let _ =
            registry.update_session_status(&record.project_id, &record.session_id, "invalid", now);
    }
    Ok(matches!(
        validation.status.as_str(),
        "passed" | "warning" | "skipped"
    ))
}

fn stored_session_validation_allows_routing(record: &AgentSessionRecord) -> bool {
    let Ok(metadata) = serde_json::from_str::<serde_json::Value>(&record.metadata_json) else {
        return false;
    };
    let Some(validation) = metadata
        .get("validation")
        .and_then(serde_json::Value::as_object)
    else {
        return false;
    };
    let status = validation.get("status").and_then(serde_json::Value::as_str);
    if !matches!(status, Some("passed" | "warning" | "skipped")) {
        return false;
    }
    let expected_root = validation
        .get("expectedRootSessionId")
        .and_then(serde_json::Value::as_str);
    let actual_root = validation
        .get("actualRootSessionId")
        .and_then(serde_json::Value::as_str);
    if expected_root != Some(record.root_session_id.as_str())
        || actual_root != Some(record.root_session_id.as_str())
    {
        return false;
    }
    let expected_role = validation
        .get("expectedRole")
        .and_then(serde_json::Value::as_str);
    let actual_role = validation
        .get("actualRole")
        .and_then(serde_json::Value::as_str);
    expected_role == actual_role && actual_role.is_some()
}

pub(super) fn required_non_empty<'a>(
    value: Option<&'a str>,
    name: &str,
) -> Result<&'a str, String> {
    let value = value.ok_or_else(|| format!("asp agent session requires {name}"))?;
    if value.trim().is_empty() {
        Err(format!("{name} must not be empty"))
    } else {
        Ok(value.trim())
    }
}
