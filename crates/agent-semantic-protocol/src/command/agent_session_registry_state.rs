use agent_semantic_client_db::{
    AgentSessionRecord, AgentSessionRegistry, agent_session_message_target_is_live_bound,
    agent_session_unix_timestamp,
};
use agent_semantic_runtime::{
    current_agent_runtime_root_session_id, current_agent_runtime_session,
    has_current_agent_runtime_session, state_core::resolve_state_home,
};

use super::agent_session_registry_validation::validate_session_profile;
use std::path::Path;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ResidentChildIdentityProof {
    CodexHookPayload,
    RegistryExact,
    CodexTranscriptRegistryExact,
    CodexRolloutMetadata,
}

pub(crate) fn registered_root_session_id(
    project_root: &Path,
    session_id: &str,
) -> Result<Option<String>, String> {
    let Some(registry) = open_existing_registry(project_root)? else {
        return Ok(None);
    };
    let project_id = project_session_scope_id(&registry, project_root)?;
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
    let project_id = project_session_scope_id(&registry, project_root)?;
    let Some(record) = registry.session_by_id(&project_id, &session.id)? else {
        return Ok(None);
    };
    registry.refresh_expired_sessions()?;
    let now = agent_session_unix_timestamp()?;
    if !record.is_routable_at(now) {
        return Ok(None);
    }
    Ok(Some(record))
}

pub(crate) fn current_registered_session_identity(
    project_root: &Path,
) -> Result<Option<AgentSessionRecord>, String> {
    let Some(session) = current_agent_runtime_session() else {
        return Ok(None);
    };
    let Some(registry) = open_existing_registry(project_root)? else {
        return Ok(None);
    };
    let project_id = project_session_scope_id(&registry, project_root)?;
    registry.session_by_id(&project_id, &session.id)
}

pub(crate) fn current_resident_child_identity_proof(
    project_root: &Path,
    resident_child_name: &str,
    resident_agent_role: &str,
) -> Result<Option<ResidentChildIdentityProof>, String> {
    let Some(session) = current_agent_runtime_session() else {
        return Ok(None);
    };
    let registry = open_existing_registry(project_root)?;
    let project_id = registry
        .as_ref()
        .map(|registry| project_session_scope_id(registry, project_root))
        .transpose()?;
    if let (Some(registry), Some(project_id)) = (registry.as_ref(), project_id.as_deref())
        && let Some(record) = registry.session_by_id(project_id, &session.id)?
    {
        return Ok(
            (record.name == resident_child_name || record.role == resident_agent_role)
                .then_some(ResidentChildIdentityProof::RegistryExact),
        );
    }

    let Some(metadata) = agent_semantic_runtime::codex_rollout_session_metadata(&session.id)?
    else {
        return Ok(None);
    };
    let Some(root_session_id) = metadata.root_session_id.as_deref() else {
        return Ok(None);
    };
    if root_session_id == session.id
        || !super::agent_session_registry_validation::rollout_metadata_matches_managed_agent_profile(
            resident_child_name,
            resident_agent_role,
            &metadata,
        )
    {
        return Ok(None);
    }
    if let (Some(registry), Some(project_id)) = (registry.as_ref(), project_id.as_deref())
        && registry
            .session_by_name(project_id, root_session_id, resident_child_name)?
            .is_some()
    {
        return Ok(None);
    }
    Ok(Some(ResidentChildIdentityProof::CodexRolloutMetadata))
}

pub(crate) fn transcript_resident_child_identity_proof(
    project_root: &Path,
    transcript_session_id: &str,
    root_session_id: &str,
    resident_child_name: &str,
    resident_agent_role: &str,
) -> Result<Option<ResidentChildIdentityProof>, String> {
    let Some(registry) = open_existing_registry(project_root)? else {
        return Ok(None);
    };
    let project_id = project_session_scope_id(&registry, project_root)?;
    let Some(record) = registry.session_by_id(&project_id, transcript_session_id)? else {
        return Ok(None);
    };
    Ok((record.root_session_id == root_session_id
        && record.session_id != record.root_session_id
        && (record.name == resident_child_name || record.role == resident_agent_role))
        .then_some(ResidentChildIdentityProof::CodexTranscriptRegistryExact))
}

pub(crate) fn codex_transcript_resident_child_identity_proof(
    project_root: &Path,
    payload: &serde_json::Value,
    resident_child_name: &str,
    resident_agent_role: &str,
) -> Result<Option<ResidentChildIdentityProof>, String> {
    let root_session_id = payload_string_field(payload, &["session_id", "sessionId"]);
    let transcript_session_id =
        payload_string_field(payload, &["transcript_path", "transcriptPath"])
            .and_then(|path| codex_transcript_session_id(&path));
    let (Some(transcript_session_id), Some(root_session_id)) =
        (transcript_session_id.as_deref(), root_session_id.as_deref())
    else {
        return Ok(None);
    };
    if transcript_session_id == root_session_id {
        return Ok(None);
    }
    transcript_resident_child_identity_proof(
        project_root,
        transcript_session_id,
        root_session_id,
        resident_child_name,
        resident_agent_role,
    )
}

fn payload_string_field(payload: &serde_json::Value, names: &[&str]) -> Option<String> {
    names.iter().find_map(|name| {
        payload
            .get(*name)
            .and_then(serde_json::Value::as_str)
            .map(ToOwned::to_owned)
    })
}

fn codex_transcript_session_id(path: &str) -> Option<String> {
    let stem = Path::new(path).file_stem()?.to_str()?;
    let candidate = stem.get(stem.len().checked_sub(36)?..)?;
    let valid = candidate.len() == 36
        && candidate
            .chars()
            .enumerate()
            .all(|(index, character)| match index {
                8 | 13 | 18 | 23 => character == '-',
                _ => character.is_ascii_hexdigit(),
            });
    valid.then(|| candidate.to_ascii_lowercase())
}

pub(crate) fn has_current_agent_session() -> bool {
    has_current_agent_runtime_session()
}

pub(crate) fn current_root_session_id() -> Option<String> {
    current_agent_runtime_root_session_id()
}

pub(crate) fn current_agent_session_id() -> Option<String> {
    non_empty_session_env("CODEX_THREAD_ID")
        .or_else(|| non_empty_session_env("CLAUDE_SESSION_ID"))
        .or_else(|| current_agent_runtime_session().map(|session| session.id))
}

fn non_empty_session_env(name: &str) -> Option<String> {
    std::env::var(name)
        .ok()
        .filter(|value| !value.trim().is_empty())
}

pub(crate) fn registered_resident_session_for_root(
    project_root: &Path,
    root_session_id: &str,
    session_name: &str,
) -> Result<Option<AgentSessionRecord>, String> {
    let Some(registry) = open_existing_registry(project_root)? else {
        return Ok(None);
    };
    let project_id = project_session_scope_id(&registry, project_root)?;
    let Some(record) = registry.session_by_name(&project_id, &root_session_id, session_name)?
    else {
        return Ok(None);
    };
    let now = agent_session_unix_timestamp()?;
    if !record.is_routable_at(now)
        || !agent_session_message_target_is_live_bound(&record, root_session_id)
        || !session_record_validation_allows_routing(&registry, &record, now)?
    {
        return Ok(None);
    }
    Ok(Some(record))
}

pub(super) fn open_existing_registry(
    _project_root: &Path,
) -> Result<Option<AgentSessionRegistry>, String> {
    AgentSessionRegistry::open_existing_state_root(resolve_state_home()?)
}

pub(super) fn open_or_create_default_registry(
    _project_root: &Path,
) -> Result<AgentSessionRegistry, String> {
    AgentSessionRegistry::open_or_create_state_root(resolve_state_home()?)
}

pub(super) fn current_project_session_scope_id(
    registry: &AgentSessionRegistry,
) -> Result<String, String> {
    if let Some(project_id) = current_agent_project_scope_id(registry)? {
        return Ok(project_id);
    }
    AgentSessionRegistry::current_project_scope_id()
}

pub(super) fn project_session_scope_id(
    registry: &AgentSessionRegistry,
    project_root: &Path,
) -> Result<String, String> {
    if let Some(project_id) = current_agent_project_scope_id(registry)? {
        return Ok(project_id);
    }
    Ok(AgentSessionRegistry::project_scope_id(project_root))
}

pub(super) fn current_recall_session_id(
    registry: &AgentSessionRegistry,
) -> Result<Option<String>, String> {
    let Some(session) = current_agent_runtime_session() else {
        return Ok(None);
    };
    let project_id = current_project_session_scope_id(registry)?;
    if let Some(record) = registry.session_by_id(&project_id, &session.id)? {
        return Ok(Some(record.root_session_id));
    }
    if let Some(root_session_id) = current_agent_runtime_root_session_id() {
        return Ok(Some(root_session_id));
    }
    Ok(Some(session.recall_session_id().to_string()))
}

fn current_agent_project_scope_id(
    registry: &AgentSessionRegistry,
) -> Result<Option<String>, String> {
    if let Some(session) = current_agent_runtime_session()
        && let Some(record) = registry.session_by_id_any_project(&session.id)?
    {
        return Ok(Some(record.project_id));
    }
    if let Some(root_session_id) = current_agent_runtime_root_session_id() {
        return registry.project_id_for_root_session_id(&root_session_id);
    }
    Ok(None)
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
    validation
        .get("actualRole")
        .and_then(serde_json::Value::as_str)
        .is_some()
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
