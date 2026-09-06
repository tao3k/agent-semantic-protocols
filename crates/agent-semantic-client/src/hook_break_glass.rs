// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-only

//! One-shot, state-bound Hook break-glass capabilities.

use serde::Deserialize;
use serde::Serialize;
use std::fs::OpenOptions;
use std::fs::{self};
use std::io::Write;
use std::path::Path;
use std::path::PathBuf;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;

pub(crate) const CAPABILITY_SCHEMA_ID: &str =
    "agent.semantic-protocols.hook-break-glass-capability";
pub(crate) const CAPABILITY_SCHEMA_VERSION: &str = "1";
pub(crate) const MAX_CAPABILITY_TTL_SECONDS: u64 = 60;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct HookBreakGlassCapability {
    pub schema_id: String,
    pub schema_version: String,
    pub nonce: String,
    pub workspace_root: String,
    pub root_session_id: String,
    pub protected_command_digest: String,
    pub deny_evidence_ref: String,
    pub defect_kind: String,
    pub issued_at_unix_ms: u64,
    pub expires_at_unix_ms: u64,
}

pub(crate) struct HookBreakGlassIssue<'a> {
    pub workspace_root: &'a Path,
    pub root_session_id: &'a str,
    pub protected_command: &'a str,
    pub deny_evidence_ref: &'a str,
    pub defect_kind: &'a str,
    pub ttl_seconds: u64,
}

pub(crate) fn protected_command_digest(command: &str) -> String {
    format!("blake3-256:{}", blake3::hash(command.as_bytes()).to_hex())
}

pub(crate) fn issue_hook_break_glass_capability(
    request: HookBreakGlassIssue<'_>,
) -> Result<HookBreakGlassCapability, String> {
    let state_root = break_glass_root()?;
    issue_hook_break_glass_capability_at(request, &state_root, unix_time_ms()?)
}

fn issue_hook_break_glass_capability_at(
    request: HookBreakGlassIssue<'_>,
    state_root: &Path,
    now: u64,
) -> Result<HookBreakGlassCapability, String> {
    if request.ttl_seconds == 0 || request.ttl_seconds > MAX_CAPABILITY_TTL_SECONDS {
        return Err(format!(
            "break-glass TTL must be between 1 and {MAX_CAPABILITY_TTL_SECONDS} seconds"
        ));
    }
    if request.root_session_id.trim().is_empty()
        || request.protected_command.trim().is_empty()
        || request.deny_evidence_ref.trim().is_empty()
    {
        return Err("break-glass capability bindings must be non-empty".to_owned());
    }
    let workspace_root = canonical_workspace(request.workspace_root)?;
    let expires_at_unix_ms = now
        .checked_add(request.ttl_seconds.saturating_mul(1000))
        .ok_or_else(|| "break-glass expiry overflow".to_owned())?;
    let nonce = capability_nonce(
        now,
        &workspace_root,
        request.root_session_id,
        request.protected_command,
        request.deny_evidence_ref,
    );
    let capability = HookBreakGlassCapability {
        schema_id: CAPABILITY_SCHEMA_ID.to_owned(),
        schema_version: CAPABILITY_SCHEMA_VERSION.to_owned(),
        nonce,
        workspace_root: workspace_root.to_string_lossy().into_owned(),
        root_session_id: request.root_session_id.to_owned(),
        protected_command_digest: protected_command_digest(request.protected_command),
        deny_evidence_ref: request.deny_evidence_ref.to_owned(),
        defect_kind: request.defect_kind.to_owned(),
        issued_at_unix_ms: now,
        expires_at_unix_ms,
    };
    let pending_dir = state_root.join("pending");
    create_private_dir(&pending_dir)?;
    let path = pending_dir.join(format!("{}.json", capability.nonce));
    let bytes = serde_json::to_vec(&capability)
        .map_err(|error| format!("encode break-glass capability: {error}"))?;
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)
        .map_err(|error| format!("create one-shot break-glass capability: {error}"))?;
    set_private_file_permissions(&path)?;
    file.write_all(&bytes)
        .and_then(|()| file.sync_all())
        .map_err(|error| format!("persist one-shot break-glass capability: {error}"))?;
    Ok(capability)
}

fn break_glass_root() -> Result<PathBuf, String> {
    agent_semantic_runtime::resolve_state_home()
        .map(|state_home| {
            agent_semantic_artifacts::StateHomeLayout::new(state_home)
                .runtime_state()
                .serving()
                .hook_break_glass_mailbox()
        })
        .map_err(|error| format!("resolve break-glass State Home: {error}"))
}

fn canonical_workspace(path: &Path) -> Result<PathBuf, String> {
    fs::canonicalize(path).map_err(|error| {
        format!(
            "canonicalize break-glass workspace {}: {error}",
            path.display()
        )
    })
}

fn unix_time_ms() -> Result<u64, String> {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| format!("resolve break-glass wall clock: {error}"))?
        .as_millis();
    u64::try_from(millis).map_err(|_| "break-glass wall clock exceeds u64".to_owned())
}

fn capability_nonce(
    now: u64,
    workspace_root: &Path,
    root_session_id: &str,
    protected_command: &str,
    evidence_ref: &str,
) -> String {
    let mut hasher = blake3::Hasher::new();
    hasher.update(&now.to_le_bytes());
    hasher.update(&std::process::id().to_le_bytes());
    hasher.update(workspace_root.to_string_lossy().as_bytes());
    hasher.update(root_session_id.as_bytes());
    hasher.update(protected_command.as_bytes());
    hasher.update(evidence_ref.as_bytes());
    hasher.finalize().to_hex().to_string()
}

fn create_private_dir(path: &Path) -> Result<(), String> {
    fs::create_dir_all(path)
        .map_err(|error| format!("create break-glass state directory: {error}"))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o700))
            .map_err(|error| format!("secure break-glass state directory: {error}"))?;
    }
    Ok(())
}

fn set_private_file_permissions(path: &Path) -> Result<(), String> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o600))
            .map_err(|error| format!("secure break-glass capability: {error}"))?;
    }
    Ok(())
}
