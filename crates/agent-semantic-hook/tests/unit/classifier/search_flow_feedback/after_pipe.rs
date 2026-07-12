use agent_semantic_hook::{DecisionKind, append_hook_event_state, classify_hook};
use serde_json::json;
use std::fs;

use super::{
    allowed_command_decision, allowed_prompt_decision, runtime_for_project, temp_project_root,
};

#[test]
fn pre_tool_allows_exact_selector_code_query_before_pipe_as_direct_fetch() {
    let project_root = temp_project_root("asp-hook-query-selector-before-pipe");
    append_hook_event_state(
        &project_root,
        &allowed_command_decision(
            "claude",
            "post-tool",
            "session-effect",
            "transcript-effect.jsonl",
            "asp typescript search prime --workspace . --view seeds",
        ),
    )
    .expect("write prime event");
    let mut runtime = runtime_for_project(&project_root);
    let provider = runtime
        .providers
        .iter_mut()
        .find(|provider| provider.language_id == "typescript")
        .expect("typescript fixture provider");
    provider.language_id = "gerbil-scheme".to_string();
    provider.provider_id = "gerbil-scheme-harness".to_string();
    provider.source_extensions = vec![".ss".to_string()];

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
                "command": "asp typescript query --selector typescript://packages/effect/src/Fiber.ts#item/function/runFiber --workspace . --code"
            }
        }),
    );

    assert_eq!(decision.decision, DecisionKind::Allow);
    assert!(!decision.fields.contains_key("hookFeedback"));
    let _ = fs::remove_dir_all(project_root);
}

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
            "asp typescript search prime --workspace . --view seeds",
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
            "asp typescript search pipe 'Effect concurrency Fiber' --workspace . --view seeds",
        ),
    )
    .expect("write pipe event");
    let mut runtime = runtime_for_project(&project_root);
    let provider = runtime
        .providers
        .iter_mut()
        .find(|provider| provider.language_id == "typescript")
        .expect("typescript fixture provider");
    provider.language_id = "gerbil-scheme".to_string();
    provider.provider_id = "gerbil-scheme-harness".to_string();
    provider.source_extensions = vec![".ss".to_string()];

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
                "command": "asp typescript search pipe 'concurrency' --workspace . --view seeds"
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
fn pre_tool_allows_first_search_pipe_after_new_user_prompt_boundary() {
    let project_root = temp_project_root("asp-hook-pipe-new-user-prompt");
    append_hook_event_state(
        &project_root,
        &allowed_command_decision(
            "claude",
            "post-tool",
            "session-effect",
            "transcript-effect.jsonl",
            "asp typescript search prime --workspace . --view seeds",
        ),
    )
    .expect("write old prime event");
    append_hook_event_state(
        &project_root,
        &allowed_command_decision(
            "claude",
            "post-tool",
            "session-effect",
            "transcript-effect.jsonl",
            "asp typescript search pipe 'Effect concurrency Fiber' --workspace . --view seeds",
        ),
    )
    .expect("write old pipe event");
    append_hook_event_state(
        &project_root,
        &allowed_prompt_decision("claude", "session-effect", "transcript-effect.jsonl"),
    )
    .expect("write prompt boundary");
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
                "command": "asp typescript search pipe 'fresh prompt question' --workspace . --view seeds"
            }
        }),
    );

    assert_eq!(decision.decision, DecisionKind::Allow);
    assert!(!decision.fields.contains_key("hookFeedback"));
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
            "asp typescript search pipe 'Effect concurrency Fiber' --workspace . --view seeds",
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
fn pre_tool_allows_selector_code_query_after_pipe() {
    let project_root = temp_project_root("asp-hook-query-selector-after-pipe");
    append_hook_event_state(
        &project_root,
        &allowed_command_decision(
            "claude",
            "post-tool",
            "session-effect",
            "transcript-effect.jsonl",
            "asp typescript search pipe 'Effect concurrency Fiber' --workspace . --view seeds",
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
                "command": "asp typescript query --selector typescript://packages/effect/src/Fiber.ts#item/function/runFiber --workspace . --code"
            }
        }),
    );

    assert_eq!(decision.decision, DecisionKind::Allow);
    assert!(!decision.fields.contains_key("hookFeedback"));
    let _ = fs::remove_dir_all(project_root);
}

#[test]
fn pre_tool_allows_file_selector_code_query_after_pipe() {
    let project_root = temp_project_root("asp-hook-file-selector-code-after-pipe");
    append_hook_event_state(
        &project_root,
        &allowed_command_decision(
            "claude",
            "post-tool",
            "session-effect",
            "transcript-effect.jsonl",
            "asp typescript search pipe 'Effect concurrency Fiber' --workspace . --view seeds",
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
    assert!(decision.routes.is_empty());
    let _ = fs::remove_dir_all(project_root);
}

#[test]
fn pre_tool_does_not_parse_apply_patch_text_as_search_flow_command() {
    let project_root = temp_project_root("asp-hook-apply-patch-text-after-pipe");
    append_hook_event_state(
        &project_root,
        &allowed_command_decision(
            "codex",
            "post-tool",
            "session-effect",
            "transcript-effect.jsonl",
            "asp typescript search pipe 'Effect concurrency Fiber' --workspace . --view seeds",
        ),
    )
    .expect("write pipe event");
    let runtime = runtime_for_project(&project_root);
    let embedded_command = format!(
        "asp typescript search {} 'concurrency' --workspace . --view seeds",
        "pipe"
    );
    let patch = format!(
        r#"*** Begin Patch
*** Add File: notes.txt
+{embedded_command}
*** End Patch"#
    );

    let decision = classify_hook(
        &runtime,
        "codex",
        "pre-tool",
        &json!({
            "hook_event_name": "PreToolUse",
            "session_id": "session-effect",
            "transcript_path": "transcript-effect.jsonl",
            "tool_name": "apply_patch",
            "tool_input": {
                "patch": patch
            }
        }),
    );

    assert_eq!(decision.decision, DecisionKind::Allow);
    assert!(!decision.fields.contains_key("hookFeedback"));
    let _ = fs::remove_dir_all(project_root);
}
