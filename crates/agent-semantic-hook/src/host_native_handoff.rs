use std::fs::OpenOptions;
use std::fs::{self};
use std::io::Write;
use std::path::Path;
use std::path::PathBuf;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;

use serde::Deserialize;
use serde::Serialize;
use serde_json::Value;

const CAPABILITY_SCHEMA_ID: &str = "agent.semantic-protocols.hook-host-native-handoff";
const CAPABILITY_SCHEMA_VERSION: &str = "1";
const DEFERRED_SCHEMA_ID: &str = "agent.semantic-protocols.host-native-execution-required";
const RECEIPT_KIND: &str = "asp-testing-execution-v1";
const TESTING_AGENT: &str = "asp_testing";
const REASON_KIND: &str = "host-local-ipc-permission-denied";
const RETRY_POLICY: &str = "do-not-retry-in-current-sandbox";
const EXECUTION_AUTHORITY: &str = "host-native";
// This is a stale-capability upper bound, not a polling or supervision delay.
// Cross-Agent delivery spans model turns, so the one-shot/root/argv bindings
// provide the primary authority boundary while this bound guarantees cleanup.
const CAPABILITY_TTL_MILLIS: u64 = 5 * 60 * 1000;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct HookHostNativeHandoffCapability {
    pub schema_id: String,
    pub schema_version: String,
    pub nonce: String,
    pub workspace_root: String,
    pub root_session_id: String,
    pub issuer_session_id: String,
    pub issuer_agent: String,
    pub receipt_kind: String,
    pub deferred_schema_id: String,
    pub reason_kind: String,
    pub execution_authority: String,
    pub retry_policy: String,
    pub argv: Vec<String>,
    pub command_digest: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub consumer_turn_id: Option<String>,
    pub issued_at_unix_ms: u64,
    pub expires_at_unix_ms: u64,
}

pub enum HookHostNativeHandoffEvaluation {
    Authorized(HookHostNativeHandoffCapability),
    Rejected(String),
    NotRequested,
}

pub fn publish_from_post_tool_payload(
    payload: &Value,
) -> Result<Option<HookHostNativeHandoffCapability>, String> {
    let Some(receipt) = payload
        .get("tool_response")
        .or_else(|| payload.get("toolResponse"))
        .and_then(find_deferred_receipt)
    else {
        return Ok(None);
    };
    validate_deferred_receipt(&receipt)?;
    require_verified_testing_context(payload)?;

    let command = hook_payload_command(payload)
        .ok_or_else(|| "host-native handoff PostToolUse payload requires command".to_owned())?;
    let argv = canonical_shell_argv(command)?;
    let receipt_argv = json_string_array(receipt.get("argv"), "deferred receipt argv")?;
    if argv != receipt_argv {
        return Err(
            "host-native handoff deferred argv does not match PostToolUse input".to_owned(),
        );
    }
    let command_digest = argv_digest(&argv)?;
    if receipt.get("commandDigest").and_then(Value::as_str) != Some(command_digest.as_str()) {
        return Err("host-native handoff deferred command digest mismatch".to_owned());
    }

    let workspace_root = canonical_workspace(payload)?;
    let root_session_id = required_string(payload, "root_session_id")?;
    let issuer_session_id = required_string(payload, "child_session_id")?;
    let now = unix_time_ms()?;
    let nonce = format!(
        "{}",
        blake3::hash(
            format!(
                "{now}\0{}\0{root_session_id}\0{issuer_session_id}\0{command_digest}",
                workspace_root.display()
            )
            .as_bytes()
        )
        .to_hex()
    );
    let capability = HookHostNativeHandoffCapability {
        schema_id: CAPABILITY_SCHEMA_ID.to_owned(),
        schema_version: CAPABILITY_SCHEMA_VERSION.to_owned(),
        nonce,
        workspace_root: workspace_root.to_string_lossy().into_owned(),
        root_session_id: root_session_id.to_owned(),
        issuer_session_id: issuer_session_id.to_owned(),
        issuer_agent: TESTING_AGENT.to_owned(),
        receipt_kind: RECEIPT_KIND.to_owned(),
        deferred_schema_id: DEFERRED_SCHEMA_ID.to_owned(),
        reason_kind: REASON_KIND.to_owned(),
        execution_authority: EXECUTION_AUTHORITY.to_owned(),
        retry_policy: RETRY_POLICY.to_owned(),
        argv,
        command_digest,
        consumer_turn_id: None,
        issued_at_unix_ms: now,
        expires_at_unix_ms: now + CAPABILITY_TTL_MILLIS,
    };
    persist_pending(&capability)?;
    Ok(Some(capability))
}

pub fn evaluate_hook_phase(input: &[u8], event: &str) -> HookHostNativeHandoffEvaluation {
    match evaluate_hook_phase_inner(input, event) {
        Ok(None) => HookHostNativeHandoffEvaluation::NotRequested,
        Ok(Some(capability)) => HookHostNativeHandoffEvaluation::Authorized(capability),
        Err(error) => HookHostNativeHandoffEvaluation::Rejected(error),
    }
}

fn evaluate_hook_phase_inner(
    input: &[u8],
    event: &str,
) -> Result<Option<HookHostNativeHandoffCapability>, String> {
    if !matches!(event, "pre-tool" | "permission-request") {
        return Ok(None);
    }
    let payload: Value = serde_json::from_slice(input)
        .map_err(|error| format!("host-native handoff payload must be JSON: {error}"))?;
    let Some(command) = hook_payload_command(&payload) else {
        return Ok(None);
    };
    let argv = canonical_shell_argv(command)?;
    let command_digest = argv_digest(&argv)?;
    let workspace_root = canonical_workspace(&payload)?;
    let root_session_id = payload
        .get("session_id")
        .or_else(|| payload.get("sessionId"))
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| "host-native handoff PreToolUse payload requires session_id".to_owned())?;
    let key = capability_key(&workspace_root, root_session_id, &command_digest);
    let state_root = state_root()?;
    let source_dir = if event == "pre-tool" {
        "pending"
    } else {
        "admitted"
    };
    let source_path = state_root.join(source_dir).join(format!("{key}.json"));
    if !source_path.is_file() {
        return Ok(None);
    }
    let bytes = fs::read(&source_path)
        .map_err(|error| format!("read pending host-native handoff: {error}"))?;
    let mut capability: HookHostNativeHandoffCapability = serde_json::from_slice(&bytes)
        .map_err(|error| format!("decode pending host-native handoff: {error}"))?;
    validate_capability(
        &capability,
        &workspace_root,
        root_session_id,
        &argv,
        &command_digest,
        unix_time_ms()?,
    )?;
    let turn_id = payload
        .get("turn_id")
        .or_else(|| payload.get("turnId"))
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| "host-native handoff requires turn_id".to_owned())?;
    if event == "pre-tool" {
        if capability.consumer_turn_id.is_some() {
            return Err("pending host-native handoff already has a consumer".to_owned());
        }
        capability.consumer_turn_id = Some(turn_id.to_owned());
        let admitted_dir = state_root.join("admitted");
        create_private_dir(&admitted_dir)?;
        let admitted_path = admitted_dir.join(format!("{key}.json"));
        persist_new(&admitted_path, &capability)?;
        fs::remove_file(&source_path)
            .map_err(|error| format!("retire pending host-native handoff: {error}"))?;
    } else {
        if capability.consumer_turn_id.as_deref() != Some(turn_id) {
            return Err("host-native handoff PermissionRequest turn_id mismatch".to_owned());
        }
        let consumed_dir = state_root.join("consumed");
        create_private_dir(&consumed_dir)?;
        let consumed_path = consumed_dir.join(format!("{key}.json"));
        fs::rename(&source_path, &consumed_path)
            .map_err(|error| format!("atomically consume host-native handoff: {error}"))?;
    }
    Ok(Some(capability))
}

fn validate_deferred_receipt(receipt: &Value) -> Result<(), String> {
    let valid = receipt.get("schemaId").and_then(Value::as_str) == Some(DEFERRED_SCHEMA_ID)
        && receipt.get("schemaVersion").and_then(Value::as_str) == Some("1")
        && receipt.get("state").and_then(Value::as_str) == Some("deferred")
        && receipt.get("reasonKind").and_then(Value::as_str) == Some(REASON_KIND)
        && receipt.get("executionAuthority").and_then(Value::as_str) == Some(EXECUTION_AUTHORITY)
        && receipt.get("retryPolicy").and_then(Value::as_str) == Some(RETRY_POLICY);
    if valid {
        Ok(())
    } else {
        Err("host-native handoff deferred receipt contract mismatch".to_owned())
    }
}

fn require_verified_testing_context(payload: &Value) -> Result<(), String> {
    let role = payload
        .get("agent_role")
        .or_else(|| payload.get("agentRole"))
        .and_then(Value::as_str);
    if role != Some(TESTING_AGENT) {
        return Err(
            "host-native handoff requires Config-selected asp_testing Agent role".to_owned(),
        );
    }
    Ok(())
}

fn validate_capability(
    capability: &HookHostNativeHandoffCapability,
    workspace_root: &Path,
    root_session_id: &str,
    argv: &[String],
    command_digest: &str,
    now: u64,
) -> Result<(), String> {
    let valid_contract = capability.schema_id == CAPABILITY_SCHEMA_ID
        && capability.schema_version == CAPABILITY_SCHEMA_VERSION
        && capability.issuer_agent == TESTING_AGENT
        && capability.receipt_kind == RECEIPT_KIND
        && capability.deferred_schema_id == DEFERRED_SCHEMA_ID
        && capability.reason_kind == REASON_KIND
        && capability.execution_authority == EXECUTION_AUTHORITY
        && capability.retry_policy == RETRY_POLICY;
    let valid_binding = capability.workspace_root == workspace_root.to_string_lossy()
        && capability.root_session_id == root_session_id
        && capability.argv == argv
        && capability.command_digest == command_digest;
    let valid_time = capability.expires_at_unix_ms > capability.issued_at_unix_ms
        && capability.expires_at_unix_ms - capability.issued_at_unix_ms == CAPABILITY_TTL_MILLIS
        && now >= capability.issued_at_unix_ms
        && now <= capability.expires_at_unix_ms;
    if valid_contract && valid_binding && valid_time {
        Ok(())
    } else {
        Err("host-native handoff capability binding, contract, or TTL mismatch".to_owned())
    }
}

fn persist_pending(capability: &HookHostNativeHandoffCapability) -> Result<(), String> {
    let state_root = state_root()?;
    let pending_dir = state_root.join("pending");
    create_private_dir(&pending_dir)?;
    let key = capability_key(
        Path::new(&capability.workspace_root),
        &capability.root_session_id,
        &capability.command_digest,
    );
    let path = pending_dir.join(format!("{key}.json"));
    if path.exists() {
        let existing = fs::read(&path)
            .map_err(|error| format!("read existing host-native handoff: {error}"))?;
        let existing: HookHostNativeHandoffCapability = serde_json::from_slice(&existing)
            .map_err(|error| format!("decode existing host-native handoff: {error}"))?;
        if unix_time_ms()? <= existing.expires_at_unix_ms {
            return Err("an unexpired host-native handoff is already pending".to_owned());
        }
        fs::remove_file(&path)
            .map_err(|error| format!("remove expired host-native handoff: {error}"))?;
    }
    let admitted_path = state_root.join("admitted").join(format!("{key}.json"));
    if admitted_path.exists() {
        let existing = fs::read(&admitted_path)
            .map_err(|error| format!("read admitted host-native handoff: {error}"))?;
        let existing: HookHostNativeHandoffCapability = serde_json::from_slice(&existing)
            .map_err(|error| format!("decode admitted host-native handoff: {error}"))?;
        if unix_time_ms()? <= existing.expires_at_unix_ms {
            return Err("an unexpired host-native invocation is already admitted".to_owned());
        }
        fs::remove_file(&admitted_path)
            .map_err(|error| format!("remove expired admitted host-native handoff: {error}"))?;
    }
    persist_new(&path, capability)
}

fn persist_new(path: &Path, capability: &HookHostNativeHandoffCapability) -> Result<(), String> {
    let bytes = serde_json::to_vec(capability)
        .map_err(|error| format!("encode host-native handoff: {error}"))?;
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|error| format!("create host-native handoff state: {error}"))?;
    set_private_file_permissions(path)?;
    file.write_all(&bytes)
        .and_then(|()| file.sync_all())
        .map_err(|error| format!("persist host-native handoff: {error}"))
}

fn canonical_shell_argv(command: &str) -> Result<Vec<String>, String> {
    let stages = agent_semantic_shell_parser::parse_bash_command_candidates(command)
        .map_err(|error| format!("parse host-native handoff command: {error}"))?;
    if stages.len() != 1 {
        return Err("host-native handoff requires exactly one shell stage".to_owned());
    }
    let words = stages[0].words();
    let asp_indices = words
        .iter()
        .enumerate()
        .filter_map(|(index, word)| {
            (word.rsplit(['/', '\\']).next() == Some("asp")).then_some(index)
        })
        .collect::<Vec<_>>();
    let [asp_index] = asp_indices.as_slice() else {
        return Err("host-native handoff requires one unambiguous inner asp argv".to_owned());
    };
    let mut argv = words[*asp_index..].to_vec();
    argv[0] = "asp".to_owned();
    Ok(argv)
}

pub fn argv_digest(argv: &[String]) -> Result<String, String> {
    let bytes = serde_json::to_vec(argv)
        .map_err(|error| format!("encode host-native handoff argv: {error}"))?;
    Ok(format!("blake3-256:{}", blake3::hash(&bytes).to_hex()))
}

fn capability_key(workspace_root: &Path, root_session_id: &str, command_digest: &str) -> String {
    blake3::hash(
        format!(
            "{}\0{root_session_id}\0{command_digest}",
            workspace_root.display()
        )
        .as_bytes(),
    )
    .to_hex()
    .to_string()
}

fn find_deferred_receipt(value: &Value) -> Option<Value> {
    if value.get("schemaId").and_then(Value::as_str) == Some(DEFERRED_SCHEMA_ID) {
        return Some(value.clone());
    }
    match value {
        Value::String(text) => serde_json::from_str::<Value>(text.trim())
            .ok()
            .and_then(|parsed| find_deferred_receipt(&parsed))
            .or_else(|| {
                text.lines()
                    .rev()
                    .filter_map(|line| serde_json::from_str::<Value>(line.trim()).ok())
                    .find_map(|parsed| find_deferred_receipt(&parsed))
            }),
        Value::Array(values) => values.iter().find_map(find_deferred_receipt),
        Value::Object(values) => values.values().find_map(find_deferred_receipt),
        _ => None,
    }
}

fn json_string_array(value: Option<&Value>, label: &str) -> Result<Vec<String>, String> {
    value
        .and_then(Value::as_array)
        .ok_or_else(|| format!("host-native handoff requires {label}"))?
        .iter()
        .map(|value| {
            value
                .as_str()
                .map(ToOwned::to_owned)
                .ok_or_else(|| format!("host-native handoff {label} must contain strings"))
        })
        .collect()
}

fn required_string<'a>(payload: &'a Value, field: &str) -> Result<&'a str, String> {
    payload
        .get(field)
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| format!("host-native handoff requires verified {field}"))
}

fn hook_payload_command(payload: &Value) -> Option<&str> {
    ["/tool_input/command", "/toolInput/command", "/command"]
        .into_iter()
        .find_map(|pointer| payload.pointer(pointer).and_then(Value::as_str))
}

fn canonical_workspace(payload: &Value) -> Result<PathBuf, String> {
    let cwd = payload
        .get("cwd")
        .and_then(Value::as_str)
        .ok_or_else(|| "host-native handoff payload requires cwd".to_owned())?;
    fs::canonicalize(cwd).map_err(|error| format!("canonicalize handoff workspace: {error}"))
}

fn state_root() -> Result<PathBuf, String> {
    agent_semantic_runtime::resolve_state_home()
        .map(|state_home| {
            agent_semantic_artifacts::StateHomeLayout::new(state_home)
                .runtime_state()
                .serving()
                .hook_host_native_handoff_mailbox()
        })
        .map_err(|error| format!("resolve host-native handoff State Home: {error}"))
}

fn create_private_dir(path: &Path) -> Result<(), String> {
    fs::create_dir_all(path)
        .map_err(|error| format!("create host-native handoff state directory: {error}"))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o700))
            .map_err(|error| format!("secure host-native handoff state directory: {error}"))?;
    }
    Ok(())
}

fn set_private_file_permissions(path: &Path) -> Result<(), String> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o600))
            .map_err(|error| format!("secure host-native handoff capability: {error}"))?;
    }
    Ok(())
}

fn unix_time_ms() -> Result<u64, String> {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| format!("resolve host-native handoff wall clock: {error}"))?
        .as_millis();
    u64::try_from(millis).map_err(|_| "host-native handoff wall clock exceeds u64".to_owned())
}
