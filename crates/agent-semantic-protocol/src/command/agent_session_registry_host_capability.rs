use std::env;
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use agent_semantic_client_db::AgentSessionRegistry;
use serde::{Deserialize, Serialize};

use super::agent_session_registry_args::SessionArgs;

const SCHEMA_ID: &str = "agent.semantic-protocols.host-typed-spawn-observation";
const SCHEMA_VERSION: &str = "1";
const SOURCE: &str = "native-collaboration-spawn-agent-schema";
const HOST_TREE_SCHEMA_ID: &str = "agent.semantic-protocols.host-resident-target-observation";
const HOST_TREE_SOURCE: &str = "native-collaboration-list-agents";
const NATIVE_SUBAGENT_START_SOURCE: &str = "codex.subagent-start";
const TRUSTED_RESIDENT_HOOK_SOURCE: &str = "codex.pre-tool-resident-envelope";
const NATIVE_SUBAGENT_START_OBSERVATION_TTL_SECONDS: i64 = 300;

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct HostTypedSpawnObservation {
    pub(super) schema_id: String,
    pub(super) schema_version: String,
    pub(super) root_session_id: String,
    pub(super) resident_name: String,
    pub(super) required_field: String,
    pub(super) required_value: String,
    pub(super) field_status: String,
    pub(super) source: String,
    pub(super) schema_digest: Option<String>,
    pub(super) observed_at: i64,
    pub(super) expires_at: i64,
}

impl HostTypedSpawnObservation {
    pub(super) fn is_fresh_for(&self, root_session_id: &str, name: &str, now: i64) -> bool {
        self.schema_id == SCHEMA_ID
            && self.schema_version == SCHEMA_VERSION
            && self.root_session_id == root_session_id
            && self.resident_name == name
            && self.required_field == "agent_type"
            && self.required_value == "configured-resident-agent-type"
            && matches!(self.field_status.as_str(), "present" | "absent")
            && self.source == SOURCE
            && self.observed_at <= now
            && now <= self.expires_at
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct HostResidentTargetObservation {
    pub(super) schema_id: String,
    pub(super) schema_version: String,
    pub(super) root_session_id: String,
    pub(super) resident_name: String,
    pub(super) target_status: String,
    /// Canonical native path observed by the host, selected from hook routing
    /// configuration rather than inferred from the registry lane name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(super) canonical_target: Option<String>,
    /// `present` proves path addressability only.  Native child identity is
    /// established separately by a lifecycle receipt such as SubagentStart.
    #[serde(default = "unverified_identity_status")]
    pub(super) identity_status: String,
    pub(super) source: String,
    pub(super) observed_at: i64,
    pub(super) expires_at: i64,
}

fn unverified_identity_status() -> String {
    "unverified".to_string()
}

impl HostResidentTargetObservation {
    pub(super) fn is_fresh_for(&self, root_session_id: &str, name: &str, now: i64) -> bool {
        self.schema_id == HOST_TREE_SCHEMA_ID
            && self.schema_version == SCHEMA_VERSION
            && self.root_session_id == root_session_id
            && self.resident_name == name
            && matches!(self.target_status.as_str(), "present" | "absent")
            && matches!(
                self.source.as_str(),
                HOST_TREE_SOURCE | NATIVE_SUBAGENT_START_SOURCE | TRUSTED_RESIDENT_HOOK_SOURCE
            )
            && self.observed_at <= now
            && now <= self.expires_at
    }
}

pub(super) fn observe_host_capability(
    registry: &AgentSessionRegistry,
    args: &SessionArgs,
) -> Result<(), String> {
    if args.root_session_id.is_some()
        || args.child_session_id.is_some()
        || args.message_target_id.is_some()
        || args.parent_session_id.is_some()
    {
        return Err(
            "host capability observation derives root identity from CODEX_THREAD_ID and never accepts root or child identity flags"
                .to_string(),
        );
    }
    let root_session_id = non_empty_env("CODEX_THREAD_ID")?;
    let name = args.name.as_deref().unwrap_or("asp-explore");
    let field_status = args
        .agent_type_field
        .as_deref()
        .ok_or_else(|| "--agent-type-field present|absent is required".to_string())?;
    if !matches!(field_status, "present" | "absent") {
        return Err("--agent-type-field must be `present` or `absent`".to_string());
    }
    let observed_at = unix_timestamp()?;
    let observation = HostTypedSpawnObservation {
        schema_id: SCHEMA_ID.to_string(),
        schema_version: SCHEMA_VERSION.to_string(),
        root_session_id,
        resident_name: name.to_string(),
        required_field: "agent_type".to_string(),
        required_value: "configured-resident-agent-type".to_string(),
        field_status: field_status.to_string(),
        source: SOURCE.to_string(),
        schema_digest: args.schema_digest.clone(),
        observed_at,
        expires_at: observed_at + args.observation_ttl_seconds,
    };
    write_observation(registry, &observation)?;
    if args.json {
        println!(
            "{}",
            serde_json::to_string_pretty(&observation)
                .map_err(|error| format!("render host capability observation: {error}"))?
        );
    } else {
        println!(
            "[agent-session-host-capability] rootSession=\"{}\" name=\"{}\" field=agent_type status={} source={} expiresAt={}",
            observation.root_session_id,
            observation.resident_name,
            observation.field_status,
            observation.source,
            observation.expires_at,
        );
    }
    Ok(())
}

pub(super) fn observe_host_tree(
    registry: &AgentSessionRegistry,
    args: &SessionArgs,
) -> Result<(), String> {
    reject_identity_flags(args, "host tree observation")?;
    let root_session_id = non_empty_env("CODEX_THREAD_ID")?;
    let name = args.name.as_deref().unwrap_or("asp-explore");
    let target_status = args
        .resident_target_status
        .as_deref()
        .ok_or_else(|| "--resident-target-status present|absent is required".to_string())?;
    if !matches!(target_status, "present" | "absent") {
        return Err("--resident-target-status must be `present` or `absent`".to_string());
    }
    let canonical_target = match target_status {
        "present" => Some(
            args.canonical_target
                .as_deref()
                .ok_or_else(|| {
                    "--canonical-target is required when the native resident target is present"
                        .to_string()
                })?
                .to_string(),
        ),
        _ if args.canonical_target.is_some() => {
            return Err("--canonical-target is forbidden when target status is absent".to_string());
        }
        _ => None,
    };
    let observed_at = unix_timestamp()?;
    let observation = HostResidentTargetObservation {
        schema_id: HOST_TREE_SCHEMA_ID.to_string(),
        schema_version: SCHEMA_VERSION.to_string(),
        root_session_id,
        resident_name: name.to_string(),
        target_status: target_status.to_string(),
        canonical_target,
        identity_status: if target_status == "present" {
            "verified".to_string()
        } else {
            unverified_identity_status()
        },
        source: HOST_TREE_SOURCE.to_string(),
        observed_at,
        expires_at: observed_at + args.observation_ttl_seconds,
    };
    write_host_tree_observation(registry, &observation)?;
    let project_root = std::env::current_dir()
        .map_err(|error| format!("failed to read current directory: {error}"))?;
    let project_id =
        super::agent_session_registry_state::project_session_scope_id(registry, &project_root)?;
    let mut record =
        registry.lookup_session(agent_semantic_client_db::AgentSessionLookupRequest {
            project_id: &project_id,
            session_id: None,
            root_session_id: Some(observation.root_session_id.as_str()),
            name: Some(observation.resident_name.as_str()),
        })?;
    let existing_live_binding = record.as_ref().is_some_and(|existing| {
        observation.target_status == "present"
            && agent_semantic_client_db::agent_session_registry::agent_session_message_target_is_live_bound(
                existing,
                existing.root_session_id.as_str(),
            )
    });
    let mut registers_resident_child = existing_live_binding;
    if observation.target_status == "present" {
        if let Some(canonical_target) = observation.canonical_target.as_deref()
            && let Some(expected_agent_type) = canonical_codex_agent_type(canonical_target)
            && super::agent_session_registry_bootstrap::binding::maybe_bind_verified_canonical_target(
                registry,
                record.as_ref(),
                true,
                Some(canonical_target),
                expected_agent_type,
                record.as_ref().and_then(|existing| existing.model.as_deref()),
                None,
            )?
            .is_some()
        {
            registers_resident_child = true;
        }
    } else if observation.target_status == "absent" {
        let _ =
            super::agent_session_registry_bootstrap::binding::invalidate_absent_canonical_target(
                registry,
                &project_id,
                record.as_mut(),
                true,
                observed_at,
            )?;
    }
    if args.json {
        println!(
            "{}",
            serde_json::to_string_pretty(&observation)
                .map_err(|error| format!("render host tree observation: {error}"))?
        );
    } else {
        println!(
            "[agent-session-host-tree] rootSession=\"{}\" name=\"{}\" targetStatus={} canonicalTarget=\"{}\" identityStatus={} source={} expiresAt={} registersResidentChild={}",
            observation.root_session_id,
            observation.resident_name,
            observation.target_status,
            observation.canonical_target.as_deref().unwrap_or(""),
            observation.identity_status,
            observation.source,
            observation.expires_at,
            registers_resident_child,
        );
    }
    Ok(())
}

fn canonical_codex_agent_type(canonical_target: &str) -> Option<&str> {
    canonical_target
        .strip_prefix("/root/")
        .filter(|agent_type| !agent_type.is_empty() && !agent_type.contains('/'))
}

pub(super) fn fresh_host_typed_spawn_observation(
    registry: &AgentSessionRegistry,
    root_session_id: &str,
    name: &str,
    now: i64,
) -> Result<Option<HostTypedSpawnObservation>, String> {
    let path = observation_path(registry, root_session_id, name)?;
    let bytes = match fs::read(&path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(format!("read host capability observation: {error}")),
    };
    let Ok(observation) = serde_json::from_slice::<HostTypedSpawnObservation>(&bytes) else {
        return Ok(None);
    };
    Ok(observation
        .is_fresh_for(root_session_id, name, now)
        .then_some(observation))
}

pub(super) fn fresh_host_resident_target_observation(
    registry: &AgentSessionRegistry,
    root_session_id: &str,
    name: &str,
    now: i64,
) -> Result<Option<HostResidentTargetObservation>, String> {
    let path = host_tree_observation_path(registry, root_session_id, name)?;
    let bytes = match fs::read(&path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(_) => return Ok(None),
    };
    let Ok(observation) = serde_json::from_slice::<HostResidentTargetObservation>(&bytes) else {
        return Ok(None);
    };
    Ok(observation
        .is_fresh_for(root_session_id, name, now)
        .then_some(observation))
}

/// Consume one fresh host-tree `absent` observation as the lease authorizing a
/// typed resident replacement. Removing the observation is the fencing step:
/// concurrent SubagentStart hooks cannot both use the same host-tree audit to
/// evict different resident owners.
pub(in crate::command) fn consume_fresh_absent_resident_target_observation(
    registry: &AgentSessionRegistry,
    root_session_id: &str,
    name: &str,
    now: i64,
) -> Result<bool, String> {
    let Some(observation) =
        fresh_host_resident_target_observation(registry, root_session_id, name, now)?
    else {
        return Ok(false);
    };
    if observation.target_status != "absent" {
        return Ok(false);
    }
    let path = host_tree_observation_path(registry, root_session_id, name)?;
    match fs::remove_file(path) {
        Ok(()) => Ok(true),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(format!(
            "consume absent host tree observation for resident replacement: {error}"
        )),
    }
}

fn write_observation(
    registry: &AgentSessionRegistry,
    observation: &HostTypedSpawnObservation,
) -> Result<(), String> {
    let path = observation_path(
        registry,
        &observation.root_session_id,
        &observation.resident_name,
    )?;
    let parent = path
        .parent()
        .ok_or_else(|| "host capability observation path has no parent".to_string())?;
    fs::create_dir_all(parent)
        .map_err(|error| format!("create host capability observation directory: {error}"))?;
    let temporary = path.with_extension(format!("tmp-{}", std::process::id()));
    let bytes = serde_json::to_vec_pretty(observation)
        .map_err(|error| format!("encode host capability observation: {error}"))?;
    fs::write(&temporary, bytes)
        .map_err(|error| format!("write host capability observation: {error}"))?;
    fs::rename(&temporary, &path)
        .map_err(|error| format!("commit host capability observation: {error}"))
}

fn write_host_tree_observation(
    registry: &AgentSessionRegistry,
    observation: &HostResidentTargetObservation,
) -> Result<(), String> {
    let path = host_tree_observation_path(
        registry,
        &observation.root_session_id,
        &observation.resident_name,
    )?;
    let parent = path
        .parent()
        .ok_or_else(|| "host tree observation path has no parent".to_string())?;
    fs::create_dir_all(parent)
        .map_err(|error| format!("create host tree observation directory: {error}"))?;
    let temporary = path.with_extension(format!("tmp-{}", std::process::id()));
    let bytes = serde_json::to_vec_pretty(observation)
        .map_err(|error| format!("encode host tree observation: {error}"))?;
    fs::write(&temporary, bytes)
        .map_err(|error| format!("write host tree observation: {error}"))?;
    fs::rename(&temporary, &path).map_err(|error| format!("commit host tree observation: {error}"))
}

pub(in crate::command) fn record_subagent_start_target_present(
    registry: &AgentSessionRegistry,
    root_session_id: &str,
    resident_name: &str,
    observed_at: i64,
) -> Result<(), String> {
    write_host_tree_observation(
        registry,
        &HostResidentTargetObservation {
            schema_id: HOST_TREE_SCHEMA_ID.to_string(),
            schema_version: SCHEMA_VERSION.to_string(),
            root_session_id: root_session_id.to_string(),
            resident_name: resident_name.to_string(),
            target_status: "present".to_string(),
            canonical_target: None,
            identity_status: "verified".to_string(),
            source: NATIVE_SUBAGENT_START_SOURCE.to_string(),
            observed_at,
            expires_at: observed_at + NATIVE_SUBAGENT_START_OBSERVATION_TTL_SECONDS,
        },
    )
}

pub(in crate::command) fn record_trusted_resident_hook_target_present(
    registry: &AgentSessionRegistry,
    project_id: &str,
    root_session_id: &str,
    resident_name: &str,
    canonical_target: &str,
    observed_at: i64,
) -> Result<(), String> {
    write_host_tree_observation(
        registry,
        &HostResidentTargetObservation {
            schema_id: HOST_TREE_SCHEMA_ID.to_string(),
            schema_version: SCHEMA_VERSION.to_string(),
            root_session_id: root_session_id.to_string(),
            resident_name: resident_name.to_string(),
            target_status: "present".to_string(),
            canonical_target: Some(canonical_target.to_string()),
            identity_status: "verified".to_string(),
            source: TRUSTED_RESIDENT_HOOK_SOURCE.to_string(),
            observed_at,
            expires_at: observed_at + NATIVE_SUBAGENT_START_OBSERVATION_TTL_SECONDS,
        },
    )?;
    if let Some(record) =
        registry.lookup_session(agent_semantic_client_db::AgentSessionLookupRequest {
            project_id,
            session_id: None,
            root_session_id: Some(root_session_id),
            name: Some(resident_name),
        })?
    {
        let expected_agent_type = record
            .configured_agent_type
            .as_deref()
            .unwrap_or(record.role.as_str());
        let _ =
            super::agent_session_registry_bootstrap::binding::maybe_bind_verified_canonical_target(
                registry,
                Some(&record),
                true,
                Some(canonical_target),
                expected_agent_type,
                record.model.as_deref(),
                record.model.as_deref(),
            )?;
    }
    Ok(())
}

fn observation_path(
    registry: &AgentSessionRegistry,
    root_session_id: &str,
    name: &str,
) -> Result<PathBuf, String> {
    let parent = registry
        .db_path()
        .parent()
        .ok_or_else(|| "agent session registry path has no parent".to_string())?;
    Ok(parent.join("host-capability-observations").join(format!(
        "{}--{}.json",
        safe_component(root_session_id),
        safe_component(name)
    )))
}

fn host_tree_observation_path(
    registry: &AgentSessionRegistry,
    root_session_id: &str,
    name: &str,
) -> Result<PathBuf, String> {
    let parent = registry
        .db_path()
        .parent()
        .ok_or_else(|| "agent session registry path has no parent".to_string())?;
    Ok(parent.join("host-tree-observations").join(format!(
        "{}--{}.json",
        safe_component(root_session_id),
        safe_component(name)
    )))
}

fn reject_identity_flags(args: &SessionArgs, owner: &str) -> Result<(), String> {
    if args.root_session_id.is_some()
        || args.child_session_id.is_some()
        || args.message_target_id.is_some()
        || args.parent_session_id.is_some()
    {
        return Err(format!(
            "{owner} derives root identity from CODEX_THREAD_ID and never accepts root or child identity flags"
        ));
    }
    Ok(())
}

fn safe_component(value: &str) -> String {
    value
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || matches!(character, '-' | '_') {
                character
            } else {
                '_'
            }
        })
        .collect()
}

fn non_empty_env(name: &str) -> Result<String, String> {
    env::var(name)
        .ok()
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| format!("{name} must identify the active Codex root task"))
}

fn unix_timestamp() -> Result<i64, String> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs() as i64)
        .map_err(|error| format!("system clock precedes unix epoch: {error}"))
}
