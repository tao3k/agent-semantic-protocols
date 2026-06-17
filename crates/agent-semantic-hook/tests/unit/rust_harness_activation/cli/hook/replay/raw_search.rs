use serde_json::json;

use crate::rust_harness_activation::support::temp_project_root;

use crate::rust_harness_activation::cli::hook::support::{last_hook_event, run_hook_decision};

#[test]
fn cli_hook_replay_blocks_functions_exec_command_raw_search_to_fzf_frontier() {
    let root = temp_project_root("hook-exec-raw-search");
    let command = "rg -n --glob '*.rs' WorkflowExecution src";
    let decision = run_hook_decision(
        &root,
        "pre-tool",
        json!({ "tool_name": "functions.exec_command", "tool_input": { "cmd": command } }),
    );

    assert_eq!(decision["decision"], "deny");
    assert_eq!(decision["reasonKind"], "raw-broad-search");
    assert_eq!(decision["subject"]["toolName"], "functions.exec_command");
    assert_eq!(decision["subject"]["command"], command);
    assert_eq!(decision["routes"][0]["kind"], "fzf");
    assert_eq!(
        decision["routes"][0]["argv"],
        json!([
            "asp",
            "rust",
            "search",
            "fzf",
            "WorkflowExecution",
            "owner",
            "tests",
            "--workspace",
            ".",
            "--view",
            "seeds"
        ])
    );

    let event = last_hook_event(&root);
    assert_eq!(event["event"], "pre-tool");
    assert_eq!(event["decision"], "deny");
    assert_eq!(event["reasonKind"], "raw-broad-search");
    assert_eq!(event["subject"]["toolName"], "functions.exec_command");

    std::fs::remove_dir_all(root).expect("remove hook replay temp root");
}
