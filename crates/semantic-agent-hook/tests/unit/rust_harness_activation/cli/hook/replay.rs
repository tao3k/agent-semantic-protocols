use std::io::Write;
use std::process::{Command, Stdio};

use serde_json::{Value, json};

use crate::rust_harness_activation::support::{root_owned_rust_activation_json, temp_project_root};

use super::support::{last_hook_event, run_hook_decision};

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
    assert_eq!(decision["routes"][0]["argv"][5], "src/lib.rs:1:40");
    let event = last_hook_event(&root);
    assert_eq!(event["event"], "pre-tool");
    assert_eq!(event["decision"], "deny");
    assert_eq!(event["reasonKind"], "bulk-source-dump");
    assert_eq!(event["subject"]["toolName"], "functions.exec_command");

    std::fs::remove_dir_all(root).expect("cleanup temp project root");
}

#[test]
fn cli_hook_replay_preserves_line_ranges_for_common_source_dump_pipelines() {
    for (command, selector) in [
        ("awk 'NR>=115 && NR<=240' src/lib.rs", "src/lib.rs:115:240"),
        ("awk 'NR==42' src/lib.rs", "src/lib.rs:42:42"),
        ("head -n 40 src/lib.rs", "src/lib.rs:1:40"),
        ("head -n 240 src/lib.rs | tail -n 126", "src/lib.rs:115:240"),
        (
            "tail -n +115 src/lib.rs | head -n 126",
            "src/lib.rs:115:240",
        ),
        (
            "nl -ba src/lib.rs | sed -n '115,240p'",
            "src/lib.rs:115:240",
        ),
    ] {
        let root = temp_project_root("line-range-source-dump");
        let decision = run_hook_decision(
            &root,
            "pre-tool",
            json!({
                "tool_name": "functions.exec_command",
                "tool_input": {"cmd": command}
            }),
        );

        assert_eq!(decision["decision"], "deny", "{command}");
        assert_eq!(decision["reasonKind"], "bulk-source-dump", "{command}");
        assert_eq!(decision["subject"]["command"], command, "{command}");
        assert_eq!(
            decision["routes"][0]["providerId"], "rs-harness",
            "{command}"
        );
        assert_eq!(decision["routes"][0]["argv"][5], selector, "{command}");
    }
}

#[test]
fn cli_hook_replay_blocks_functions_exec_command_raw_search_to_query_route() {
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
    assert_eq!(decision["routes"][0]["kind"], "query");
    assert_eq!(
        decision["routes"][0]["argv"],
        json!([
            "rs-harness",
            "query",
            "--from-hook",
            "direct-source-read",
            "--selector",
            "**/*.rs",
            "--term",
            "HookDecision",
            "--surface",
            "owners,tests",
            "--view",
            "seeds",
            "."
        ])
    );
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
    assert_eq!(decision["routes"][0]["argv"][5], "src/lib.rs:1-2");
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

#[test]
fn cli_hook_fails_open_on_invalid_payload_json() {
    let root = temp_project_root("hook-invalid-payload");
    let activation_path = root.join("activation.json");
    std::fs::write(&activation_path, root_owned_rust_activation_json()).expect("write activation");
    let mut child = Command::new(env!("CARGO_BIN_EXE_semantic-agent-hook"))
        .args([
            "hook",
            "--client",
            "codex",
            "pre-tool",
            "--emit",
            "decision",
            "--activation",
            activation_path.to_str().expect("utf8 activation path"),
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
        .write_all(b"{not-json")
        .expect("write invalid hook payload");

    let output = child.wait_with_output().expect("wait for hook command");

    assert!(
        output.status.success(),
        "hook stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let decision: Value = serde_json::from_slice(&output.stdout).expect("hook decision JSON");
    assert_eq!(decision["decision"], "allow");
    assert_eq!(decision["reasonKind"], "none");
    assert!(
        decision["message"]
            .as_str()
            .unwrap()
            .contains("invalid hook payload JSON")
    );
    std::fs::remove_dir_all(root).expect("cleanup temp project root");
}
