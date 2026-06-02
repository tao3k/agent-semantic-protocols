use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};

use serde_json::{Value, json};

use crate::rust_harness_profile::support::{
    root_owned_rust_profile_registry_json, temp_project_root,
};

#[test]
fn cli_hook_replay_blocks_functions_exec_command_source_dump() {
    let root = temp_project_root("hook-exec-source-dump");
    let decision = run_hook_decision(
        &root,
        "pre-tool",
        json!({
            "tool_name": "functions.exec_command",
            "tool_input": {"cmd": "sed -n '1,40p' src/lib.rs"}
        }),
    );

    assert_eq!(decision["decision"], "deny");
    assert_eq!(decision["reasonKind"], "bulk-source-dump");
    assert_eq!(decision["subject"]["toolName"], "functions.exec_command");
    assert_eq!(decision["subject"]["command"], "sed -n '1,40p' src/lib.rs");
    assert_eq!(decision["routes"][0]["providerId"], "rs-harness");
    let event = last_hook_event(&root);
    assert_eq!(event["event"], "pre-tool");
    assert_eq!(event["decision"], "deny");
    assert_eq!(event["reasonKind"], "bulk-source-dump");
    assert_eq!(event["subject"]["toolName"], "functions.exec_command");

    std::fs::remove_dir_all(root).expect("cleanup temp project root");
}

#[test]
fn cli_hook_replay_blocks_functions_exec_command_raw_search() {
    let root = temp_project_root("hook-exec-raw-search");
    let decision = run_hook_decision(
        &root,
        "pre-tool",
        json!({
            "tool_name": "functions.exec_command",
            "tool_input": {"cmd": "rg -n HookDecision src tests"}
        }),
    );

    assert_eq!(decision["decision"], "deny");
    assert_eq!(decision["reasonKind"], "raw-broad-search");
    assert_eq!(decision["subject"]["toolName"], "functions.exec_command");
    assert_eq!(
        decision["subject"]["command"],
        "rg -n HookDecision src tests"
    );
    assert_eq!(decision["routes"][0]["kind"], "ingest");
    let event = last_hook_event(&root);
    assert_eq!(event["event"], "pre-tool");
    assert_eq!(event["decision"], "deny");
    assert_eq!(event["reasonKind"], "raw-broad-search");
    assert_eq!(event["subject"]["toolName"], "functions.exec_command");

    std::fs::remove_dir_all(root).expect("cleanup temp project root");
}

#[test]
fn cli_hook_replay_blocks_nested_parallel_exec_command() {
    let root = temp_project_root("hook-parallel-exec");
    let decision = run_hook_decision(
        &root,
        "pre-tool",
        json!({
            "tool_name": "multi_tool_use.parallel",
            "tool_input": {
                "tool_uses": [{
                    "recipient_name": "functions.exec_command",
                    "parameters": {"cmd": "rtk read src/lib.rs:1-2"}
                }]
            }
        }),
    );

    assert_eq!(decision["decision"], "deny");
    assert_eq!(decision["reasonKind"], "direct-source-read");
    assert_eq!(decision["subject"]["toolName"], "functions.exec_command");
    assert_eq!(decision["subject"]["command"], "rtk read src/lib.rs:1-2");
    assert_eq!(decision["routes"][0]["providerId"], "rs-harness");
    assert_eq!(decision["routes"][0]["argv"][0], "rs-harness");
    assert_eq!(decision["routes"][0]["argv"][1], "query");
    assert_eq!(decision["routes"][0]["argv"][5], "src/lib.rs");
    let event = last_hook_event(&root);
    assert_eq!(event["event"], "pre-tool");
    assert_eq!(event["decision"], "deny");
    assert_eq!(event["reasonKind"], "direct-source-read");
    assert_eq!(event["subject"]["toolName"], "functions.exec_command");

    std::fs::remove_dir_all(root).expect("cleanup temp project root");
}

#[test]
fn cli_hook_replay_records_allow_decision_for_exec_command_post_tool() {
    let root = temp_project_root("hook-exec-allow-post-tool");
    let decision = run_hook_decision(
        &root,
        "post-tool",
        json!({
            "toolName": "functions.exec_command",
            "toolInput": {"cmd": "cargo test -p semantic-agent-hook classifier::routes"}
        }),
    );

    assert_eq!(decision["decision"], "allow");
    assert_eq!(decision["reasonKind"], "none");
    assert_eq!(decision["event"], "post-tool");
    assert_eq!(decision["subject"]["toolName"], "functions.exec_command");
    assert_eq!(
        decision["subject"]["command"],
        "cargo test -p semantic-agent-hook classifier::routes"
    );
    let event = last_hook_event(&root);
    assert_eq!(event["event"], "post-tool");
    assert_eq!(event["decision"], "allow");
    assert_eq!(event["reasonKind"], "none");
    assert_eq!(event["subject"]["toolName"], "functions.exec_command");

    std::fs::remove_dir_all(root).expect("cleanup temp project root");
}

fn run_hook_decision(root: &Path, event: &str, payload: Value) -> Value {
    let profiles_path = root.join("profiles.json");
    std::fs::write(&profiles_path, root_owned_rust_profile_registry_json())
        .expect("write profile registry");
    let mut child = Command::new(env!("CARGO_BIN_EXE_semantic-agent-hook"))
        .args([
            "hook",
            "--client",
            "codex",
            event,
            "--emit",
            "decision",
            "--profiles",
            profiles_path.to_str().expect("utf8 profiles path"),
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("run semantic-agent-hook hook");
    child
        .stdin
        .as_mut()
        .expect("hook stdin")
        .write_all(payload.to_string().as_bytes())
        .expect("write hook payload");
    let output = child.wait_with_output().expect("wait for hook command");
    assert!(
        output.status.success(),
        "hook stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).expect("hook decision JSON")
}

fn last_hook_event(root: &Path) -> Value {
    let events = std::fs::read_to_string(root.join("events.jsonl")).expect("hook event state");
    let line = events
        .lines()
        .last()
        .expect("at least one recorded hook event");
    serde_json::from_str(line).expect("hook event JSON")
}
