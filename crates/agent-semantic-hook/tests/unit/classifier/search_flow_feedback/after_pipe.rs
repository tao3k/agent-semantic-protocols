use agent_semantic_hook::{DecisionKind, append_hook_event_state, classify_hook};
use serde_json::json;
use std::fs;

use super::{allowed_command_decision, runtime_for_project, temp_project_root};

#[test]
fn pre_tool_denies_repeated_search_pipe_after_pipe() {
    let project_root = temp_project_root("asp-hook-repeat-pipe");
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
                "command": "asp typescript search pipe 'concurrency' --view seeds ."
            }
        }),
    );

    assert_eq!(decision.decision, DecisionKind::Deny);
    assert_eq!(decision.fields["hookFeedback"], "repeat-search-pipe");
    assert!(
        decision
            .message
            .contains("Follow the previous `recommendedNext`"),
        "{}",
        decision.message
    );
    let _ = fs::remove_dir_all(project_root);
}

#[test]
fn pre_tool_denies_unbounded_direct_source_read_after_pipe() {
    let project_root = temp_project_root("asp-hook-direct-read-after-pipe");
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
                "command": "asp typescript query --from-hook direct-source-read --selector packages/effect/src/Fiber.ts --workspace . --code"
            }
        }),
    );

    assert_eq!(decision.decision, DecisionKind::Deny);
    assert_eq!(
        decision.fields["hookFeedback"],
        "direct-source-read-after-pipe"
    );
    assert!(
        decision
            .message
            .contains("query --selector <path:start-end>"),
        "{}",
        decision.message
    );
    let _ = fs::remove_dir_all(project_root);
}

#[test]
fn pre_tool_allows_bounded_low_priority_direct_source_read_after_pipe() {
    let project_root = temp_project_root("asp-hook-bounded-direct-read-after-pipe");
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
                "command": "asp typescript query --from-hook direct-source-read --selector packages/effect/src/Fiber.ts:20:59 --workspace . --code"
            }
        }),
    );

    assert_eq!(decision.decision, DecisionKind::Allow);
    assert!(!decision.fields.contains_key("hookFeedback"));
    let _ = fs::remove_dir_all(project_root);
}

#[test]
fn pre_tool_denies_broad_direct_source_read_after_pipe() {
    let project_root = temp_project_root("asp-hook-broad-direct-read-after-pipe");
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
                "command": "asp typescript query --from-hook direct-source-read --selector packages/effect/src/Fiber.ts:1:120 --workspace . --code"
            }
        }),
    );

    assert_eq!(decision.decision, DecisionKind::Deny);
    assert_eq!(
        decision.fields["hookFeedback"],
        "direct-source-read-after-pipe"
    );
    assert!(
        decision.message.contains("80 lines or fewer"),
        "{}",
        decision.message
    );
    let _ = fs::remove_dir_all(project_root);
}

#[test]
fn pre_tool_allows_selector_code_query_after_pipe() {
    let project_root = temp_project_root("asp-hook-query-selector-after-pipe");
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
                "command": "asp typescript query --selector packages/effect/src/Fiber.ts:110:112 --workspace . --code"
            }
        }),
    );

    assert_eq!(decision.decision, DecisionKind::Allow);
    assert!(!decision.fields.contains_key("hookFeedback"));
    let _ = fs::remove_dir_all(project_root);
}
