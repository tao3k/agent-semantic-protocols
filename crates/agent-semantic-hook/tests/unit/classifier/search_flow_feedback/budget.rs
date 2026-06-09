use agent_semantic_hook::{DecisionKind, append_hook_event_state, classify_hook};
use serde_json::json;
use std::fs;

use super::{allowed_command_decision, runtime_for_project, temp_project_root};

#[test]
fn pre_tool_denies_asp_command_after_prompt_budget_is_exhausted() {
    let project_root = temp_project_root("asp-hook-command-budget");
    append_hook_event_state(
        &project_root,
        &allowed_command_decision(
            "claude",
            "post-tool",
            "session-effect",
            "transcript-effect.jsonl",
            "asp typescript search prime --view seeds .",
        ),
    )
    .expect("write prime event");
    append_hook_event_state(
        &project_root,
        &allowed_command_decision(
            "claude",
            "post-tool",
            "session-effect",
            "transcript-effect.jsonl",
            "asp typescript search pipe 'Effect concurrency Fiber' --view seeds .",
        ),
    )
    .expect("write pipe event");
    let runtime = runtime_for_project(&project_root);

    unsafe {
        std::env::set_var("ASP_HOOK_MAX_ASP_COMMANDS", "2");
    }
    let decision = classify_hook(
        &runtime,
        "claude",
        "pre-tool",
        &json!({
            "hook_event_name": "PreToolUse",
            "session_id": "session-effect",
            "transcript_path": "transcript-effect.jsonl",
            "tool_name": "Bash",
            "tool_input": {
                "command": "asp rg -query 'Fiber|Scope' packages/effect/src"
            }
        }),
    );
    unsafe {
        std::env::remove_var("ASP_HOOK_MAX_ASP_COMMANDS");
    }

    assert_eq!(decision.decision, DecisionKind::Deny);
    assert_eq!(
        decision.fields["hookFeedback"],
        "asp-command-budget-exhausted"
    );
    assert_eq!(decision.fields["aspCommandCount"], 2);
    assert_eq!(decision.fields["maxAspCommands"], 2);
    assert!(
        decision
            .message
            .contains("Answer from the existing ASP frontier"),
        "{}",
        decision.message
    );
    let _ = fs::remove_dir_all(project_root);
}
