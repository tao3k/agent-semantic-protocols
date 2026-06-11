use serde_json::json;

use crate::rust_harness_activation::support::temp_project_root;

use crate::rust_harness_activation::cli::hook::support::{last_hook_event, run_hook_decision};

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
    assert_eq!(decision["routes"][0]["argv"][4], "src/lib.rs:1:40");
    let event = last_hook_event(&root);
    assert_eq!(event["event"], "pre-tool");
    assert_eq!(event["decision"], "deny");
    assert_eq!(event["reasonKind"], "bulk-source-dump");
    assert_eq!(event["subject"]["toolName"], "functions.exec_command");

    std::fs::remove_dir_all(root).expect("cleanup temp project root");
}

#[test]
fn cli_hook_replay_compacts_repeated_source_dump_lane() {
    let root = temp_project_root("hook-repeated-source-dump");
    let first = run_hook_decision(
        &root,
        "pre-tool",
        json!({
            "tool_name": "functions.exec_command",
            "tool_input": {"cmd": "sed -n '1,40p' src/lib.rs"}
        }),
    );
    let second = run_hook_decision(
        &root,
        "pre-tool",
        json!({
            "tool_name": "functions.exec_command",
            "tool_input": {"cmd": "head -n 40 src/lib.rs"}
        }),
    );

    assert_eq!(first["decision"], "deny");
    assert_eq!(first["fields"]["denyReplay"], "record");
    assert_eq!(second["decision"], "deny");
    assert_eq!(second["reasonKind"], "bulk-source-dump");
    assert_eq!(second["fields"]["denyReplay"], "repeated");
    let message = second["message"].as_str().expect("replay message");
    assert!(message.starts_with("ASP hook already denied `bulk-source-dump`"));
    assert!(message.contains("Follow the previous recovery route"));
    assert!(!message.contains("## Agent Flow"));
    assert_eq!(second["routes"][0]["argv"][4], "src/lib.rs:1:40");

    let event = last_hook_event(&root);
    assert_eq!(event["fields"]["denyReplay"], "repeated");
    assert!(
        event["denyReplayKey"]
            .as_str()
            .is_some_and(|key| !key.is_empty())
    );

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
        assert_eq!(decision["routes"][0]["argv"][4], selector, "{command}");
    }
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
    assert_eq!(decision["routes"][0]["argv"][0], "asp");
    assert_eq!(decision["routes"][0]["argv"][1], "rust");
    assert_eq!(decision["routes"][0]["argv"][2], "query");
    assert_eq!(decision["routes"][0]["argv"][4], "src/lib.rs:1-2");
    let event = last_hook_event(&root);
    assert_eq!(event["event"], "pre-tool");
    assert_eq!(event["decision"], "deny");
    assert_eq!(event["reasonKind"], "direct-source-read");
    assert_eq!(event["subject"]["toolName"], "functions.exec_command");

    std::fs::remove_dir_all(root).expect("cleanup temp project root");
}
