//! Durable configured-Agent route selected by the standalone Hook evaluator.

use std::path::Path;
use std::path::PathBuf;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;

use serde::Deserialize;
use serde::Serialize;

use crate::aot_evaluator::AotHookDecision;

const SCHEMA_ID: &str = "agent.semantic-protocols.hook-session-route";
const SCHEMA_VERSION: u32 = 1;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AotHookSessionRouteReceipt {
    pub schema_id: String,
    pub schema_version: u32,
    pub recorded_at_unix_millis: u128,
    pub root_session_id: String,
    pub workspace_root: PathBuf,
    pub config_rule_id: String,
    pub target_agent: String,
    pub reason_kind: String,
    pub command_digest: Option<String>,
    pub subject_command: Option<String>,
}

pub fn publish_aot_hook_session_route(
    project_root: &Path,
    decision: &AotHookDecision<'_>,
    payload: &serde_json::Value,
) -> Result<AotHookSessionRouteReceipt, String> {
    let root_session_id = decision.session_id.ok_or_else(|| {
        "hook-session-id-required: configured Agent deny is missing the Host session identity"
            .to_owned()
    })?;
    let target_agent = decision.route.ok_or_else(|| {
        "hook-session-route-required: configured Agent deny is missing its route".to_owned()
    })?;
    let workspace_root = std::fs::canonicalize(project_root).map_err(|error| {
        format!(
            "canonicalize configured Agent deny workspace {}: {error}",
            project_root.display()
        )
    })?;
    let subject_command = payload
        .pointer("/tool_input/command")
        .or_else(|| payload.pointer("/tool_input/cmd"))
        .and_then(serde_json::Value::as_str)
        .map(str::to_owned);
    let command_digest = subject_command
        .as_deref()
        .map(|command| format!("blake3-256:{}", blake3::hash(command.as_bytes()).to_hex()));
    let receipt = AotHookSessionRouteReceipt {
        schema_id: SCHEMA_ID.to_owned(),
        schema_version: SCHEMA_VERSION,
        recorded_at_unix_millis: unix_time_millis(),
        root_session_id: root_session_id.to_owned(),
        workspace_root,
        config_rule_id: decision.config_rule_id.to_owned(),
        target_agent: target_agent.to_owned(),
        reason_kind: decision.reason_kind.to_owned(),
        command_digest,
        subject_command,
    };
    let path = session_route_path(root_session_id)?;
    let parent = path
        .parent()
        .ok_or_else(|| format!("Hook session route has no parent: {}", path.display()))?;
    std::fs::create_dir_all(parent)
        .map_err(|error| format!("create Hook session route directory: {error}"))?;
    let staged = parent.join(format!(".route-{}.tmp", std::process::id()));
    let bytes = serde_json::to_vec(&receipt)
        .map_err(|error| format!("encode Hook session route receipt: {error}"))?;
    std::fs::write(&staged, bytes)
        .map_err(|error| format!("stage Hook session route receipt: {error}"))?;
    std::fs::rename(&staged, &path)
        .map_err(|error| format!("publish Hook session route receipt: {error}"))?;
    Ok(receipt)
}

pub fn read_aot_hook_session_route(
    root_session_id: &str,
    project_root: &Path,
) -> Result<Option<AotHookSessionRouteReceipt>, String> {
    let path = session_route_path(root_session_id)?;
    let bytes = match std::fs::read(&path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => {
            return Err(format!(
                "read Hook session route receipt {}: {error}",
                path.display()
            ));
        }
    };
    let receipt: AotHookSessionRouteReceipt = serde_json::from_slice(&bytes)
        .map_err(|error| format!("decode Hook session route receipt: {error}"))?;
    if receipt.schema_id != SCHEMA_ID || receipt.schema_version != SCHEMA_VERSION {
        return Err("hook-session-route-schema-invalid: expected schema version 1".to_owned());
    }
    if receipt.root_session_id != root_session_id {
        return Err("hook-session-route-root-binding-mismatch".to_owned());
    }
    let workspace_root = std::fs::canonicalize(project_root).map_err(|error| {
        format!(
            "canonicalize collaboration workspace {}: {error}",
            project_root.display()
        )
    })?;
    if receipt.workspace_root != workspace_root {
        return Err("hook-session-route-workspace-binding-mismatch".to_owned());
    }
    Ok(Some(receipt))
}

fn session_route_path(root_session_id: &str) -> Result<PathBuf, String> {
    if root_session_id.is_empty() {
        return Err("hook-session-id-required: empty Host session identity".to_owned());
    }
    let state_home = std::env::var_os("ASP_STATE_HOME")
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .or_else(|| {
            std::env::var_os("HOME")
                .filter(|value| !value.is_empty())
                .map(|home| PathBuf::from(home).join(".agent-semantic-protocols"))
        })
        .ok_or_else(|| "Hook session route requires ASP_STATE_HOME or HOME".to_owned())?;
    let key = blake3::hash(root_session_id.as_bytes()).to_hex();
    Ok(state_home
        .join("hooks/session-routes")
        .join(format!("{key}.json")))
}

fn unix_time_millis() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or_default()
}
