// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use super::inspect_host_rollout;
use serde_json::json;
use std::io::Write;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;

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
    probe_call_with_id("probe-call")
}

fn probe_call_with_id(call_id: &str) -> serde_json::Value {
    json!({
        "type": "response_item",
        "payload": {
            "type": "function_call",
            "name": "exec_command",
            "call_id": call_id,
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
                "text": "<hook_prompt>[asp-hook] {\"schemaId\":\"agent.semantic-protocols.hook.decision\",\"schemaVersion\":\"1\",\"event\":\"pre-tool\",\"decision\":\"deny\",\"fields\":{\"configRuleId\":\"route-read-to-asp-languages\",\"hookMatcherGeneration\":\"blake3-256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\",\"hookPolicySnapshotDigest\":\"blake3-256:policy\",\"hookRuntimeArtifactFingerprint\":\"blake3-256:artifact\"}}</hook_prompt>"
            }]
        }
    })
}

fn hook_event(call_id: &str, generation_bound: bool) -> serde_json::Value {
    let mut policy = json!({
        "schemaId": "agent.semantic-protocols.hook.decision",
        "schemaVersion": 1,
        "decision": "deny",
        "configRuleId": "route-read-to-asp-languages",
    });
    if generation_bound {
        policy["generationDigest"] = json!(format!("blake3-256:{}", "a".repeat(64)));
    }
    json!({
        "schemaId": "agent.semantic-protocols.hook.event",
        "schemaVersion": "1",
        "event": "pre-tool",
        "decision": "deny",
        "reasonKind": "registered-source-route-required",
        "fields": {
            "hostMatcher": "Bash",
            "sessionId": "normal-task-session",
            "toolUseId": call_id,
            "policyDecision": policy,
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
    let events = write_rollout("accepted-events", &[hook_event("probe-call", true)]);
    let path = write_rollout(
        "accepted",
        &[
            world_state(true),
            probe_call(),
            hook_deny(),
            probe_output(""),
        ],
    );
    let receipt =
        inspect_host_rollout(&path, &events, PROBE_PATH, SENTINEL).expect("inspect rollout");
    assert!(receipt.accepted(), "{receipt:?}");
    assert_eq!(
        receipt.reason_kind(),
        "normal-task-hook-policy-bundle-bound-deny-observed"
    );
    std::fs::remove_file(path).expect("remove rollout");
    std::fs::remove_file(events).expect("remove Hook events");
}

#[test]
fn hook_event_is_plugin_load_provenance_when_world_state_has_no_plugin_instructions() {
    let events = write_rollout("hook-load-events", &[hook_event("probe-call", true)]);
    let path = write_rollout(
        "hook-load",
        &[
            world_state(false),
            probe_call(),
            hook_deny(),
            probe_output(""),
        ],
    );
    let receipt =
        inspect_host_rollout(&path, &events, PROBE_PATH, SENTINEL).expect("inspect rollout");
    assert!(receipt.accepted(), "{receipt:?}");
    std::fs::remove_file(path).expect("remove rollout");
    std::fs::remove_file(events).expect("remove Hook events");
}

#[test]
fn arbitrary_deny_without_publication_identity_is_rejected() {
    let events = write_rollout("unbound-events", &[hook_event("probe-call", false)]);
    let unbound_deny = json!({
        "type": "response_item",
        "payload": {
            "type": "message",
            "role": "user",
            "content": [{
                "type": "input_text",
                "text": "<hook_prompt>[asp-hook] {\"schemaId\":\"agent.semantic-protocols.hook.decision\",\"schemaVersion\":\"1\",\"event\":\"pre-tool\",\"decision\":\"deny\"}</hook_prompt>"
            }]
        }
    });
    let path = write_rollout(
        "unbound-deny",
        &[
            world_state(true),
            probe_call(),
            unbound_deny,
            probe_output(""),
        ],
    );
    let receipt =
        inspect_host_rollout(&path, &events, PROBE_PATH, SENTINEL).expect("inspect rollout");
    assert!(!receipt.accepted(), "{receipt:?}");
    assert_eq!(
        receipt.reason_kind(),
        "hook-deny-publication-identity-missing"
    );
    std::fs::remove_file(path).expect("remove rollout");
    std::fs::remove_file(events).expect("remove Hook events");
}

#[test]
fn unified_exec_deny_without_source_bytes_is_accepted() {
    let events = write_rollout(
        "unified-exec-events",
        &[hook_event("custom-probe-call", true)],
    );
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
    let receipt =
        inspect_host_rollout(&path, &events, PROBE_PATH, SENTINEL).expect("inspect rollout");
    assert!(receipt.accepted(), "{receipt:?}");
    assert_eq!(
        receipt.reason_kind(),
        "normal-task-hook-policy-bundle-bound-deny-observed"
    );
    std::fs::remove_file(path).expect("remove rollout");
    std::fs::remove_file(events).expect("remove Hook events");
}

#[test]
fn unified_exec_source_bytes_leak_is_correlated_to_the_probe_call() {
    let events = write_rollout(
        "unified-exec-source-leak-events",
        &[hook_event("custom-probe-call", true)],
    );
    let path = write_rollout(
        "unified-exec-source-leak",
        &[
            world_state(true),
            custom_probe_call(),
            hook_deny(),
            custom_probe_output("custom-probe-call", SENTINEL),
        ],
    );
    let receipt =
        inspect_host_rollout(&path, &events, PROBE_PATH, SENTINEL).expect("inspect rollout");
    assert!(!receipt.accepted(), "{receipt:?}");
    assert_eq!(receipt.reason_kind(), "source-bytes-leaked");
    std::fs::remove_file(path).expect("remove rollout");
    std::fs::remove_file(events).expect("remove Hook events");
}

#[test]
fn plugin_disabled_normal_task_is_rejected_even_when_source_read_was_called() {
    let events = write_rollout("plugin-disabled-events", &[]);
    let path = write_rollout(
        "plugin-disabled",
        &[world_state(false), probe_call(), probe_output(SENTINEL)],
    );
    let receipt =
        inspect_host_rollout(&path, &events, PROBE_PATH, SENTINEL).expect("inspect rollout");
    assert!(!receipt.accepted(), "{receipt:?}");
    assert_eq!(receipt.reason_kind(), "plugin-not-loaded");
    std::fs::remove_file(path).expect("remove rollout");
    std::fs::remove_file(events).expect("remove Hook events");
}

#[test]
fn source_bytes_leak_rejects_even_when_hook_event_and_deny_are_present() {
    let events = write_rollout("source-leak-events", &[hook_event("probe-call", true)]);
    let path = write_rollout(
        "source-leak",
        &[
            world_state(true),
            probe_call(),
            hook_deny(),
            probe_output(SENTINEL),
        ],
    );
    let receipt =
        inspect_host_rollout(&path, &events, PROBE_PATH, SENTINEL).expect("inspect rollout");
    assert!(!receipt.accepted(), "{receipt:?}");
    assert_eq!(receipt.reason_kind(), "source-bytes-leaked");
    std::fs::remove_file(path).expect("remove rollout");
    std::fs::remove_file(events).expect("remove Hook events");
}

#[test]
fn long_rollout_uses_bounded_prefix_and_tail_windows() {
    let events = write_rollout("bounded-tail-events", &[hook_event("probe-call", true)]);
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

    let receipt =
        inspect_host_rollout(&path, &events, PROBE_PATH, SENTINEL).expect("inspect rollout");
    assert!(receipt.accepted(), "{receipt:?}");
    assert_eq!(
        receipt.reason_kind(),
        "normal-task-hook-policy-bundle-bound-deny-observed"
    );
    std::fs::remove_file(path).expect("remove rollout");
    std::fs::remove_file(events).expect("remove Hook events");
}

#[test]
fn unrelated_hook_deny_does_not_cover_missing_probe_delivery() {
    let events = write_rollout("unrelated-events", &[hook_event("other-call", true)]);
    let path = write_rollout(
        "missing-exact-delivery",
        &[
            world_state(true),
            probe_call(),
            hook_deny(),
            probe_output(""),
        ],
    );
    let receipt =
        inspect_host_rollout(&path, &events, PROBE_PATH, SENTINEL).expect("inspect rollout");
    assert!(!receipt.accepted(), "{receipt:?}");
    assert_eq!(receipt.reason_kind(), "hook-event-missing");
    std::fs::remove_file(path).expect("remove rollout");
    std::fs::remove_file(events).expect("remove Hook events");
}

#[test]
fn one_denied_retry_does_not_cover_another_missing_probe_delivery() {
    let events = write_rollout("partial-retry-events", &[hook_event("probe-call", true)]);
    let path = write_rollout(
        "partial-retry-delivery",
        &[
            world_state(true),
            probe_call(),
            probe_call_with_id("probe-retry-call"),
            hook_deny(),
            probe_output(""),
        ],
    );
    let receipt =
        inspect_host_rollout(&path, &events, PROBE_PATH, SENTINEL).expect("inspect rollout");
    assert!(!receipt.accepted(), "{receipt:?}");
    assert_eq!(receipt.reason_kind(), "hook-event-missing");
    std::fs::remove_file(path).expect("remove rollout");
    std::fs::remove_file(events).expect("remove Hook events");
}

#[test]
fn receipt_matches_the_shared_schema() {
    let events = write_rollout("schema-events", &[hook_event("probe-call", true)]);
    let path = write_rollout(
        "schema",
        &[
            world_state(true),
            probe_call(),
            hook_deny(),
            probe_output(""),
        ],
    );
    let receipt =
        inspect_host_rollout(&path, &events, PROBE_PATH, SENTINEL).expect("inspect rollout");
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
    std::fs::remove_file(events).expect("remove Hook events");
}
