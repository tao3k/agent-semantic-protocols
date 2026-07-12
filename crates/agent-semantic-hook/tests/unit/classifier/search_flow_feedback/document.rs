use agent_semantic_hook::{DecisionKind, append_hook_event_state, classify_hook};
use serde_json::json;
use std::fs;

use super::{allowed_command_decision, document_runtime_for_project, temp_project_root};

#[test]
fn pre_tool_allows_document_explicit_read_after_pipe() {
    let project_root = temp_project_root("asp-hook-read-after-pipe-md");
    append_hook_event_state(
        &project_root,
        &allowed_command_decision(
            "claude",
            "post-tool",
            "session-effect",
            "transcript-effect.jsonl",
            "asp md search prime --workspace . --view seeds",
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
            "asp md search pipe 'document query projection' --workspace . --view seeds",
        ),
    )
    .expect("write pipe event");
    let runtime = document_runtime_for_project(&project_root);

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
                "command": "asp md query --from-hook direct-source-read --selector README.md --workspace ."
            }
        }),
    );

    assert_eq!(decision.decision, DecisionKind::Allow);
    assert!(decision.routes.is_empty());
    let _ = fs::remove_dir_all(project_root);
}
