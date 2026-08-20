use super::inspect_host_rollout;
use serde_json::json;
use std::io::Write;
use std::time::{SystemTime, UNIX_EPOCH};

const SENTINEL: &str = "ASP_NORMAL_TASK_SOURCE_SENTINEL_7F21";
const PROBE_PATH: &str = "tests/fixtures/hook-host-acceptance/probe.rs";

fn rollout_path(name: &str) -> std::path::PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    std::env::temp_dir().join(format!("asp-hook-host-{name}-{unique}.jsonl"))
}

fn write_rollout(name: &str, values: &[serde_json::Value]) -> std::path::PathBuf {
    let path = rollout_path(name);
    let mut file = std::fs::File::create(&path).expect("create rollout");
    for value in values {
        writeln!(file, "{value}").expect("write rollout item");
    }
    path
}

fn world_state(plugin_loaded: bool) -> serde_json::Value {
    json!({
        "type": "world_state",
        "payload": { "state": { "plugins_instructions": plugin_loaded } }
    })
}

fn probe_call() -> serde_json::Value {
    json!({
        "type": "response_item",
        "payload": {
            "type": "function_call",
            "name": "exec_command",
            "call_id": "probe-call",
            "arguments": format!(r#"{{"cmd":"sed -n '1,5p' {PROBE_PATH}"}}"#)
        }
    })
}

fn hook_deny() -> serde_json::Value {
    json!({
        "type": "response_item",
        "payload": {
            "type": "message",
            "role": "user",
            "content": [{
                "type": "input_text",
                "text": "<hook_prompt>[asp-hook] {\"schemaId\":\"agent.semantic-protocols.hook.decision\",\"decision\":\"deny\"}</hook_prompt>"
            }]
        }
    })
}

fn probe_output(output: &str) -> serde_json::Value {
    json!({
        "type": "response_item",
        "payload": {
            "type": "function_call_output",
            "call_id": "probe-call",
            "output": output
        }
    })
}

fn custom_probe_call() -> serde_json::Value {
    json!({
        "type": "response_item",
        "payload": {
            "type": "custom_tool_call",
            "name": "exec",
            "call_id": "custom-probe-call",
            "input": format!(
                r#"const result = await tools.exec_command({{cmd: \"sed -n '1,5p' {PROBE_PATH}\"}});"#
            )
        }
    })
}

fn custom_probe_output(call_id: &str, output: &str) -> serde_json::Value {
    json!({
        "type": "response_item",
        "payload": {
            "type": "custom_tool_call_output",
            "call_id": call_id,
            "output": [{"type": "input_text", "text": output}]
        }
    })
}

#[test]
fn normal_task_deny_without_source_bytes_is_accepted() {
    let path = write_rollout(
        "accepted",
        &[
            world_state(true),
            probe_call(),
            hook_deny(),
            probe_output(""),
        ],
    );
    let receipt = inspect_host_rollout(&path, PROBE_PATH, SENTINEL).expect("inspect rollout");
    assert!(receipt.accepted(), "{receipt:?}");
    assert_eq!(receipt.reason_kind(), "normal-task-hook-deny-observed");
    std::fs::remove_file(path).expect("remove rollout");
}

#[test]
fn unified_exec_deny_without_source_bytes_is_accepted() {
    let path = write_rollout(
        "unified-exec-accepted",
        &[
            world_state(true),
            custom_probe_call(),
            hook_deny(),
            custom_probe_output("unrelated-call", SENTINEL),
            custom_probe_output("custom-probe-call", "Command blocked by PreToolUse hook"),
        ],
    );
    let receipt = inspect_host_rollout(&path, PROBE_PATH, SENTINEL).expect("inspect rollout");
    assert!(receipt.accepted(), "{receipt:?}");
    assert_eq!(receipt.reason_kind(), "normal-task-hook-deny-observed");
    std::fs::remove_file(path).expect("remove rollout");
}

#[test]
fn unified_exec_source_bytes_leak_is_correlated_to_the_probe_call() {
    let path = write_rollout(
        "unified-exec-source-leak",
        &[
            world_state(true),
            custom_probe_call(),
            hook_deny(),
            custom_probe_output("custom-probe-call", SENTINEL),
        ],
    );
    let receipt = inspect_host_rollout(&path, PROBE_PATH, SENTINEL).expect("inspect rollout");
    assert!(!receipt.accepted(), "{receipt:?}");
    assert_eq!(receipt.reason_kind(), "source-bytes-leaked");
    std::fs::remove_file(path).expect("remove rollout");
}

#[test]
fn plugin_disabled_normal_task_is_rejected_even_when_source_read_was_called() {
    let path = write_rollout(
        "plugin-disabled",
        &[world_state(false), probe_call(), probe_output(SENTINEL)],
    );
    let receipt = inspect_host_rollout(&path, PROBE_PATH, SENTINEL).expect("inspect rollout");
    assert!(!receipt.accepted(), "{receipt:?}");
    assert_eq!(receipt.reason_kind(), "plugin-not-loaded");
    std::fs::remove_file(path).expect("remove rollout");
}

#[test]
fn source_bytes_leak_rejects_even_when_hook_event_and_deny_are_present() {
    let path = write_rollout(
        "source-leak",
        &[
            world_state(true),
            probe_call(),
            hook_deny(),
            probe_output(SENTINEL),
        ],
    );
    let receipt = inspect_host_rollout(&path, PROBE_PATH, SENTINEL).expect("inspect rollout");
    assert!(!receipt.accepted(), "{receipt:?}");
    assert_eq!(receipt.reason_kind(), "source-bytes-leaked");
    std::fs::remove_file(path).expect("remove rollout");
}

#[test]
fn long_rollout_uses_bounded_prefix_and_tail_windows() {
    let path = write_rollout("bounded-tail", &[world_state(true)]);
    let mut file = std::fs::OpenOptions::new()
        .append(true)
        .open(&path)
        .expect("open rollout for history");
    let padding = "x".repeat(1024 * 1024);
    for _ in 0..65 {
        writeln!(file, "{}", json!({ "type": "history", "payload": padding }))
            .expect("write history record");
    }
    for value in [probe_call(), hook_deny(), probe_output("")] {
        writeln!(file, "{value}").expect("write trailing acceptance evidence");
    }
    drop(file);

    let receipt = inspect_host_rollout(&path, PROBE_PATH, SENTINEL).expect("inspect rollout");
    assert!(receipt.accepted(), "{receipt:?}");
    assert_eq!(receipt.reason_kind(), "normal-task-hook-deny-observed");
    std::fs::remove_file(path).expect("remove rollout");
}

#[test]
fn receipt_matches_the_shared_schema() {
    let path = write_rollout(
        "schema",
        &[
            world_state(true),
            probe_call(),
            hook_deny(),
            probe_output(""),
        ],
    );
    let receipt = inspect_host_rollout(&path, PROBE_PATH, SENTINEL).expect("inspect rollout");
    let schema: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../../schemas/semantic-hook-host-acceptance.v1.schema.json"
    ))
    .expect("parse receipt schema");
    let instance = serde_json::to_value(receipt).expect("serialize receipt");
    jsonschema::validator_for(&schema)
        .expect("compile receipt schema")
        .validate(&instance)
        .expect("validate receipt");
    std::fs::remove_file(path).expect("remove rollout");
}
