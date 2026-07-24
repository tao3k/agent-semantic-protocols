use serde_json::json;

use crate::rust_harness_activation::support::temp_project_root;

use crate::rust_harness_activation::cli::hook::support::{last_hook_event, run_hook_decision};

#[test]
fn cli_hook_replay_matches_uncontrolled_source_command_rule_to_resident_asp_explore() {
    let root = temp_project_root("hook-exec-raw-search");
    let command = "rg -n --glob '*.rs' WorkflowExecution src";
    let decision = run_hook_decision(
        &root,
        "pre-tool",
        json!({ "tool_name": "functions.exec_command", "tool_input": { "cmd": command } }),
    );

    assert_eq!(decision["decision"], "deny");
    assert_eq!(decision["reasonKind"], "raw-broad-search");
    assert_eq!(
        decision["fields"]["configRuleId"],
        "deny-uncontrolled-source-search-commands"
    );
    assert_eq!(decision["subject"]["toolName"], "functions.exec_command");
    assert_eq!(decision["subject"]["command"], command);
    assert_eq!(decision["routes"], json!([]));
    assert_eq!(decision["fields"]["targetAgentName"], "asp_explorer");
    assert_eq!(decision["fields"]["targetAgentRole"], "asp_explorer");
    assert_eq!(
        decision["fields"]["requiredAction"],
        "enter-asp-explore-choice-pane"
    );
    assert_eq!(
        decision["fields"]["forbiddenUntilResolved"],
        "raw-source-fallback"
    );

    let event = last_hook_event(&root);
    assert_eq!(event["event"], "pre-tool");
    assert_eq!(event["decision"], "deny");
    assert_eq!(event["reasonKind"], "raw-broad-search");
    assert_eq!(event["subject"]["toolName"], "functions.exec_command");

    std::fs::remove_dir_all(root).expect("remove hook replay temp root");
}
