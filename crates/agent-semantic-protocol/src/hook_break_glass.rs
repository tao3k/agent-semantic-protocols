//! One-shot, state-bound Hook break-glass capabilities.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

pub(crate) const CAPABILITY_SCHEMA_ID: &str =
    "agent.semantic-protocols.hook-break-glass-capability";
pub(crate) const CAPABILITY_SCHEMA_VERSION: &str = "1";
pub(crate) const MAX_CAPABILITY_TTL_SECONDS: u64 = 60;
const BREAK_GLASS_ENV: &str = "ASP_BREAK_GLASS_CAPABILITY";

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

pub(crate) enum HookBreakGlassEvaluation {
    NotRequested,
    Authorized(HookBreakGlassCapability),
    Rejected(String),
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

pub(crate) fn evaluate_hook_break_glass(input: &[u8]) -> HookBreakGlassEvaluation {
    match evaluate_hook_break_glass_inner(input) {
        Ok(None) => HookBreakGlassEvaluation::NotRequested,
        Ok(Some(capability)) => HookBreakGlassEvaluation::Authorized(capability),
        Err(error) => HookBreakGlassEvaluation::Rejected(error),
    }
}

fn evaluate_hook_break_glass_inner(
    input: &[u8],
) -> Result<Option<HookBreakGlassCapability>, String> {
    let state_root = break_glass_root()?;
    let inherited_request = (std::env::var_os("ASP_NO_AGENT").as_deref()
        == Some(std::ffi::OsStr::new("1")))
    .then(|| std::env::var(BREAK_GLASS_ENV))
    .transpose()
    .map_err(|_| {
        "naked ASP_NO_AGENT request rejected: ASP_BREAK_GLASS_CAPABILITY is required".to_owned()
    })?;
    evaluate_hook_break_glass_inner_at(
        input,
        &state_root,
        unix_time_ms()?,
        inherited_request.as_deref(),
    )
}

fn evaluate_hook_break_glass_inner_at(
    input: &[u8],
    state_root: &Path,
    now: u64,
    inherited_capability_nonce: Option<&str>,
) -> Result<Option<HookBreakGlassCapability>, String> {
    let payload: Value = serde_json::from_slice(input)
        .map_err(|error| format!("break-glass hook payload must be JSON: {error}"))?;
    let command = hook_payload_command(&payload).unwrap_or_default();
    let request = if let Some(nonce) = inherited_capability_nonce {
        Some((nonce.to_owned(), command.to_owned()))
    } else {
        inline_break_glass_request(command)?
    };
    let Some((nonce, protected_command)) = request else {
        return Ok(None);
    };
    validate_nonce(&nonce)?;
    if protected_command.trim().is_empty() {
        return Err("break-glass protected command must be non-empty".to_owned());
    }
    let pending_path = state_root.join("pending").join(format!("{nonce}.json"));
    let bytes = fs::read(&pending_path)
        .map_err(|error| format!("read pending break-glass capability: {error}"))?;
    let capability = serde_json::from_slice::<HookBreakGlassCapability>(&bytes)
        .map_err(|error| format!("decode pending break-glass capability: {error}"))?;
    validate_capability(&capability, &nonce, &protected_command, &payload, now)?;
    let consumed_dir = state_root.join("consumed");
    create_private_dir(&consumed_dir)?;
    let consumed_path = consumed_dir.join(format!("{nonce}.json"));
    fs::rename(&pending_path, &consumed_path)
        .map_err(|error| format!("atomically consume break-glass capability: {error}"))?;
    append_consumption_audit(state_root, &capability, now)?;
    Ok(Some(capability))
}

fn validate_capability(
    capability: &HookBreakGlassCapability,
    nonce: &str,
    protected_command: &str,
    payload: &Value,
    now: u64,
) -> Result<(), String> {
    if capability.schema_id != CAPABILITY_SCHEMA_ID
        || capability.schema_version != CAPABILITY_SCHEMA_VERSION
        || capability.nonce != nonce
    {
        return Err("break-glass capability schema or nonce mismatch".to_owned());
    }
    let ttl = capability
        .expires_at_unix_ms
        .checked_sub(capability.issued_at_unix_ms)
        .ok_or_else(|| "break-glass capability expiry precedes issue time".to_owned())?;
    if ttl == 0
        || ttl > MAX_CAPABILITY_TTL_SECONDS * 1000
        || now < capability.issued_at_unix_ms
        || now > capability.expires_at_unix_ms
    {
        return Err("break-glass capability is expired or has an invalid TTL".to_owned());
    }
    if capability.protected_command_digest != protected_command_digest(protected_command) {
        return Err("break-glass protected command digest mismatch".to_owned());
    }
    let payload_root = payload
        .get("cwd")
        .and_then(Value::as_str)
        .ok_or_else(|| "break-glass Hook payload requires cwd".to_owned())?;
    let payload_root = canonical_workspace(Path::new(payload_root))?;
    if capability.workspace_root != payload_root.to_string_lossy() {
        return Err("break-glass workspace binding mismatch".to_owned());
    }
    let root_session_id = payload
        .get("session_id")
        .or_else(|| payload.get("sessionId"))
        .and_then(Value::as_str)
        .ok_or_else(|| "break-glass Hook payload requires root session identity".to_owned())?;
    if capability.root_session_id != root_session_id {
        return Err("break-glass root session binding mismatch".to_owned());
    }
    Ok(())
}

fn inline_break_glass_request(command: &str) -> Result<Option<(String, String)>, String> {
    let command = command.trim_start();
    let Some(rest) = strip_leading_assignment(command, "ASP_NO_AGENT=1") else {
        return Ok(None);
    };
    let rest = rest.trim_start();
    let Some(rest) = rest.strip_prefix("ASP_BREAK_GLASS_CAPABILITY=") else {
        return Err(
            "naked ASP_NO_AGENT request rejected: ASP_BREAK_GLASS_CAPABILITY is required"
                .to_owned(),
        );
    };
    let boundary = rest
        .find(char::is_whitespace)
        .ok_or_else(|| "break-glass inline request must include a protected command".to_owned())?;
    let nonce = rest[..boundary].to_owned();
    let protected_command = rest[boundary..].trim_start().to_owned();
    Ok(Some((nonce, protected_command)))
}

fn strip_leading_assignment<'a>(command: &'a str, assignment: &str) -> Option<&'a str> {
    let rest = command.strip_prefix(assignment)?;
    if rest.is_empty() || rest.chars().next().is_some_and(char::is_whitespace) {
        Some(rest)
    } else {
        None
    }
}

fn hook_payload_command(payload: &Value) -> Option<&str> {
    [
        "/tool_input/command",
        "/tool_input/cmd",
        "/toolInput/command",
        "/toolInput/cmd",
        "/input/command",
        "/input/cmd",
        "/command",
        "/cmd",
    ]
    .into_iter()
    .find_map(|pointer| payload.pointer(pointer).and_then(Value::as_str))
}

fn break_glass_root() -> Result<PathBuf, String> {
    agent_semantic_runtime::resolve_state_home()
        .map(|state_home| state_home.join("hook-break-glass"))
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

fn validate_nonce(nonce: &str) -> Result<(), String> {
    if nonce.len() == 64
        && nonce
            .bytes()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
    {
        Ok(())
    } else {
        Err("break-glass capability nonce must be 64 hexadecimal characters".to_owned())
    }
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

fn append_consumption_audit(
    state_root: &Path,
    capability: &HookBreakGlassCapability,
    consumed_at_unix_ms: u64,
) -> Result<(), String> {
    create_private_dir(state_root)?;
    let audit = state_root.join("audit.jsonl");
    let record = serde_json::json!({
        "schemaId": "agent.semantic-protocols.hook-break-glass-consumption",
        "schemaVersion": "1",
        "consumedAtUnixMs": consumed_at_unix_ms,
        "capability": capability,
    });
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&audit)
        .map_err(|error| format!("open break-glass audit: {error}"))?;
    set_private_file_permissions(&audit)?;
    writeln!(file, "{record}")
        .and_then(|()| file.flush())
        .map_err(|error| format!("append break-glass audit: {error}"))
}

#[cfg(test)]
#[path = "../tests/unit/hook_break_glass.rs"]
mod tests;
