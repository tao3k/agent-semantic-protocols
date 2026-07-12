//! Trust-state writer for Codex plugin hook declarations.

use std::{env, fs, path::Path, path::PathBuf};

use serde::Serialize;
use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::codex_trust::{TRUST_BLOCK_END, codex_trust_block_begin, merge_codex_trust_config};

/// Status of the Codex user trust entries required for plugin hooks.
#[derive(Debug)]
pub struct CodexPluginTrustStatus {
    /// User-level Codex config containing plugin hook trust hashes.
    pub trust_config_path: PathBuf,
    /// Whether all required plugin hook hashes match the current plugin hooks file.
    pub hook_state_trusted: bool,
    /// Hook state keys still missing trusted state.
    pub missing_events: Vec<String>,
    /// Hook state keys with trusted state that no longer matches the current plugin hooks file.
    pub stale_events: Vec<String>,
}

/// Install user-level Codex trust state for hooks declared by a plugin hooks file.
pub fn install_codex_user_plugin_trust_state(
    hooks_json_path: &Path,
    hook_key_source: &str,
) -> Result<PathBuf, String> {
    let hooks_json_path = fs::canonicalize(hooks_json_path).map_err(|error| {
        format!(
            "failed to resolve Codex plugin hooks file {}: {error}",
            hooks_json_path.display()
        )
    })?;
    let codex_home = codex_home_path()?;
    fs::create_dir_all(&codex_home)
        .map_err(|error| format!("failed to create {}: {error}", codex_home.display()))?;
    let user_config_path = codex_home.join("config.toml");
    let existing = fs::read_to_string(&user_config_path).unwrap_or_default();
    if user_config_path.is_file() {
        validate_codex_config_toml(&existing).map_err(|error| {
            format!(
                "refusing to write invalid Codex user config {}: {error}",
                user_config_path.display()
            )
        })?;
    }
    let trust_block = codex_plugin_trust_state_block(&hooks_json_path, hook_key_source)?;
    let merged = merge_codex_trust_config(&existing, Path::new(hook_key_source), &trust_block);
    validate_codex_config_toml(&merged).map_err(|error| {
        format!(
            "refusing to write invalid Codex user plugin trust config {}: {error}",
            user_config_path.display()
        )
    })?;
    fs::write(&user_config_path, merged.as_bytes())
        .map_err(|error| format!("failed to write {}: {error}", user_config_path.display()))?;
    Ok(user_config_path)
}

/// Inspect user-level Codex trust state for hooks declared by a plugin hooks file.
pub fn codex_user_plugin_trust_state_status(
    hooks_json_path: &Path,
    hook_key_source: &str,
) -> Result<CodexPluginTrustStatus, String> {
    let hooks_json_path = fs::canonicalize(hooks_json_path).map_err(|error| {
        format!(
            "failed to resolve Codex plugin hooks file {}: {error}",
            hooks_json_path.display()
        )
    })?;
    let trust_config_path = codex_home_path()?.join("config.toml");
    let content = fs::read_to_string(&trust_config_path).unwrap_or_default();
    let parsed =
        toml::from_str::<toml::Value>(&content)
            .unwrap_or(toml::Value::Table(
                toml::map::Map::<String, toml::Value>::new(),
            ));
    let state = parsed
        .get("hooks")
        .and_then(toml::Value::as_table)
        .and_then(|hooks| hooks.get("state"))
        .and_then(toml::Value::as_table);
    let expected = codex_plugin_expected_hook_states(&hooks_json_path, hook_key_source)?;
    let (missing_events, stale_events) = expected.into_iter().fold(
        (Vec::new(), Vec::new()),
        |(mut missing_events, mut stale_events), expected| {
            match codex_plugin_hook_trust_hash(state, &expected.key) {
                Some(actual_hash) if actual_hash == expected.trusted_hash => {}
                Some(_) => stale_events.push(expected.key),
                None => missing_events.push(expected.key),
            }
            (missing_events, stale_events)
        },
    );
    let hook_state_trusted = missing_events.is_empty() && stale_events.is_empty();
    Ok(CodexPluginTrustStatus {
        trust_config_path,
        hook_state_trusted,
        missing_events,
        stale_events,
    })
}

fn codex_plugin_trust_state_block(
    hooks_json_path: &Path,
    hook_key_source: &str,
) -> Result<String, String> {
    let state = codex_plugin_hook_state_blocks(hooks_json_path, hook_key_source)?.join("\n\n");
    Ok(format!(
        "{}\n{state}\n{TRUST_BLOCK_END}",
        codex_trust_block_begin(Path::new(hook_key_source))
    ))
}

fn codex_plugin_hook_state_blocks(
    hooks_json_path: &Path,
    hook_key_source: &str,
) -> Result<Vec<String>, String> {
    codex_plugin_expected_hook_states(hooks_json_path, hook_key_source).map(|states| {
        states
            .into_iter()
            .map(|state| {
                format!(
                    "[hooks.state.{}]\ntrusted_hash = {}",
                    toml_basic_string(&state.key),
                    toml_basic_string(&state.trusted_hash)
                )
            })
            .collect()
    })
}

#[derive(Debug)]
struct CodexPluginExpectedHookState {
    key: String,
    trusted_hash: String,
}

fn codex_plugin_expected_hook_states(
    hooks_json_path: &Path,
    hook_key_source: &str,
) -> Result<Vec<CodexPluginExpectedHookState>, String> {
    let hooks_json = fs::read_to_string(hooks_json_path)
        .map_err(|error| format!("failed to read {}: {error}", hooks_json_path.display()))?;
    let hooks_json = serde_json::from_str::<Value>(&hooks_json)
        .map_err(|error| format!("invalid Codex plugin hooks JSON: {error}"))?;
    let hooks_by_event = hooks_json
        .get("hooks")
        .and_then(Value::as_object)
        .ok_or_else(|| "Codex plugin hooks JSON must contain a `hooks` object".to_string())?;
    hooks_by_event
        .iter()
        .flat_map(|(event_name, entries)| {
            codex_plugin_event_expected_states(hook_key_source, event_name, entries)
                .unwrap_or_else(|error| vec![Err(error)])
        })
        .collect()
}

fn codex_plugin_event_expected_states(
    hook_key_source: &str,
    event_name: &str,
    entries: &Value,
) -> Result<Vec<Result<CodexPluginExpectedHookState, String>>, String> {
    let state_label = codex_plugin_event_state_label(event_name)?;
    let entries = entries
        .as_array()
        .ok_or_else(|| format!("Codex plugin hook event `{event_name}` must be an array"))?;
    entries
        .iter()
        .enumerate()
        .try_fold(Vec::new(), |mut blocks, (entry_index, entry)| {
            blocks.extend(codex_plugin_hook_expected_states(
                hook_key_source,
                state_label,
                entry_index,
                entry,
            )?);
            Ok::<_, String>(blocks)
        })
        .map(|blocks| blocks.into_iter().map(Ok).collect())
}

fn codex_plugin_hook_expected_states(
    hook_key_source: &str,
    state_label: &str,
    entry_index: usize,
    entry: &Value,
) -> Result<Vec<CodexPluginExpectedHookState>, String> {
    let entry = entry
        .as_object()
        .ok_or_else(|| "Codex plugin hook entry must be an object".to_string())?;
    let matcher = codex_plugin_event_matcher(state_label, entry);
    let hooks = entry
        .get("hooks")
        .and_then(Value::as_array)
        .ok_or_else(|| "Codex plugin hook entry must contain a `hooks` array".to_string())?;
    hooks
        .iter()
        .enumerate()
        .map(|(handler_index, hook)| {
            let normalized_handler = codex_plugin_command_hook_identity(hook)?;
            let identity = NativeNormalizedHookIdentity {
                event_name: state_label,
                matcher: matcher.clone(),
                hooks: vec![normalized_handler],
            };
            let hash = version_for_codex_toml_identity(&identity);
            let key = format!("{hook_key_source}:{state_label}:{entry_index}:{handler_index}");
            Ok(CodexPluginExpectedHookState {
                key,
                trusted_hash: hash,
            })
        })
        .collect()
}

fn codex_plugin_hook_trust_hash(
    state: Option<&toml::map::Map<String, toml::Value>>,
    key: &str,
) -> Option<String> {
    state
        .and_then(|state| state.get(key))
        .and_then(toml::Value::as_table)
        .and_then(|entry| entry.get("trusted_hash"))
        .and_then(toml::Value::as_str)
        .map(str::to_string)
}

fn codex_plugin_event_state_label(event_name: &str) -> Result<&'static str, String> {
    match event_name {
        "SessionStart" => Ok("session_start"),
        "UserPromptSubmit" => Ok("user_prompt_submit"),
        "PreToolUse" => Ok("pre_tool_use"),
        "PermissionRequest" => Ok("permission_request"),
        "PostToolUse" => Ok("post_tool_use"),
        "PreCompact" => Ok("pre_compact"),
        "PostCompact" => Ok("post_compact"),
        "SubagentStart" => Ok("subagent_start"),
        "SubagentStop" => Ok("subagent_stop"),
        "Stop" => Ok("stop"),
        other => Err(format!("unsupported Codex plugin hook event `{other}`")),
    }
}

fn codex_plugin_event_matcher(
    state_label: &str,
    entry: &serde_json::Map<String, Value>,
) -> Option<String> {
    match state_label {
        "pre_tool_use" | "permission_request" | "post_tool_use" | "session_start"
        | "subagent_start" | "subagent_stop" | "pre_compact" | "post_compact" => entry
            .get("matcher")
            .and_then(Value::as_str)
            .map(str::to_string),
        "user_prompt_submit" | "stop" => None,
        _ => None,
    }
}

#[derive(Serialize)]
struct NativeNormalizedHookIdentity<'a> {
    event_name: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    matcher: Option<String>,
    hooks: Vec<NativeHookHandlerConfig>,
}

#[derive(Serialize)]
#[serde(tag = "type")]
enum NativeHookHandlerConfig {
    #[serde(rename = "command")]
    Command {
        command: String,
        #[serde(rename = "commandWindows", skip_serializing_if = "Option::is_none")]
        command_windows: Option<String>,
        #[serde(rename = "timeout")]
        timeout_sec: u64,
        r#async: bool,
        #[serde(rename = "statusMessage", skip_serializing_if = "Option::is_none")]
        status_message: Option<String>,
    },
}

fn codex_plugin_command_hook_identity(hook: &Value) -> Result<NativeHookHandlerConfig, String> {
    let hook = hook
        .as_object()
        .ok_or_else(|| "Codex plugin command hook must be an object".to_string())?;
    let hook_type = hook
        .get("type")
        .and_then(Value::as_str)
        .unwrap_or("command");
    if hook_type != "command" {
        return Err(format!(
            "Codex plugin trust state only supports command hooks, got `{hook_type}`"
        ));
    }
    let command = hook
        .get("command")
        .and_then(Value::as_str)
        .ok_or_else(|| "Codex plugin command hook must contain `command`".to_string())?;
    let timeout_sec = hook
        .get("timeout")
        .and_then(Value::as_u64)
        .unwrap_or(600)
        .max(1);
    let status_message = hook
        .get("statusMessage")
        .and_then(Value::as_str)
        .map(str::to_string);
    Ok(NativeHookHandlerConfig::Command {
        command: command.to_string(),
        command_windows: None,
        timeout_sec,
        r#async: hook.get("async").and_then(Value::as_bool).unwrap_or(false),
        status_message,
    })
}

fn version_for_codex_toml_identity<T>(value: &T) -> String
where
    T: Serialize,
{
    let value = toml::Value::try_from(value).unwrap_or(toml::Value::String(String::new()));
    let value = serde_json::to_value(value).unwrap_or(Value::Null);
    let canonical = canonical_json(value);
    let serialized = serde_json::to_vec(&canonical).unwrap_or_default();
    let hash = Sha256::digest(serialized);
    let hex = hash
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    format!("sha256:{hex}")
}

fn canonical_json(value: Value) -> Value {
    match value {
        Value::Object(map) => {
            let mut keys = map.keys().cloned().collect::<Vec<_>>();
            keys.sort();
            let sorted = keys
                .into_iter()
                .filter_map(|key| {
                    map.get(&key)
                        .map(|value| (key, canonical_json(value.clone())))
                })
                .collect();
            Value::Object(sorted)
        }
        Value::Array(items) => Value::Array(items.into_iter().map(canonical_json).collect()),
        other => other,
    }
}

fn validate_codex_config_toml(content: &str) -> Result<(), String> {
    toml::from_str::<toml::Value>(content)
        .map(|_| ())
        .map_err(|error| error.to_string())
}

fn codex_home_path() -> Result<PathBuf, String> {
    if let Some(path) = env::var_os("CODEX_HOME").filter(|value| !value.is_empty()) {
        return Ok(PathBuf::from(path));
    }
    env::var_os("HOME")
        .filter(|value| !value.is_empty())
        .map(|home| PathBuf::from(home).join(".codex"))
        .ok_or_else(|| {
            "missing CODEX_HOME and HOME; cannot write Codex plugin hook trust state".to_string()
        })
}

fn toml_basic_string(value: &str) -> String {
    serde_json::to_string(value).expect("string serialization should not fail")
}
