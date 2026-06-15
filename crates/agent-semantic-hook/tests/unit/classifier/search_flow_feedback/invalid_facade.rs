use agent_semantic_hook::{DecisionKind, classify_hook};
use serde_json::json;
use std::fs;

use super::{runtime_for_project, temp_project_root};
use crate::classifier::{provider, provider_routes};

#[test]
fn pre_tool_denies_package_name_as_asp_facade() {
    let project_root = temp_project_root("asp-hook-invalid-facade");
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
                "command": "asp effect prime --view seeds ."
            }
        }),
    );

    assert_eq!(decision.decision, DecisionKind::Deny);
    assert_eq!(decision.language_ids, vec!["typescript"]);
    assert_eq!(decision.fields["hookFeedback"], "invalid-asp-facade");
    assert_eq!(decision.fields["invalidFacade"], "effect");
    assert_eq!(decision.fields["languageId"], "typescript");
    assert!(
        decision.message.contains("ASP facades are language IDs"),
        "{}",
        decision.message
    );
    assert!(
        decision
            .message
            .contains("asp typescript search prime --workspace . --view seeds"),
        "{}",
        decision.message
    );
    let _ = fs::remove_dir_all(project_root);
}

#[test]
fn pre_tool_denies_unknown_facade_without_unrelated_provider_recovery() {
    let project_root = temp_project_root("asp-hook-unknown-facade");
    let mut runtime = runtime_for_project(&project_root);
    runtime.providers.push(provider(
        "gerbil-scheme",
        "gerbil-scheme-harness",
        "gerbil-scheme-harness",
        "agent.semantic-protocols.languages.gerbil-scheme.gerbil-scheme-harness",
        &[".ss", ".scm"],
        &["gerbil.pkg"],
        &["src", "test"],
        &[],
        provider_routes("gerbil-scheme-harness", None),
    ));

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
                "command": "asp scheme search prime --workspace . --view seeds"
            }
        }),
    );

    assert_eq!(decision.decision, DecisionKind::Deny);
    assert!(decision.language_ids.is_empty());
    assert_eq!(decision.fields["hookFeedback"], "invalid-asp-facade");
    assert_eq!(decision.fields["invalidFacade"], "scheme");
    assert!(!decision.fields.contains_key("languageId"));
    assert!(
        decision.message.contains("asp providers"),
        "{}",
        decision.message
    );
    assert!(
        decision.message.contains("asp fd -query"),
        "{}",
        decision.message
    );
    assert!(
        decision
            .message
            .contains("Do not switch to an unrelated active facade"),
        "{}",
        decision.message
    );
    assert!(
        !decision
            .message
            .contains("asp typescript search prime --workspace . --view seeds"),
        "{}",
        decision.message
    );
    assert!(
        !decision
            .message
            .contains("asp gerbil-scheme search prime --workspace . --view seeds"),
        "{}",
        decision.message
    );
    assert!(
        !decision.message.contains("Suggested matching facade"),
        "{}",
        decision.message
    );
    let _ = fs::remove_dir_all(project_root);
}

#[test]
fn pre_tool_allows_root_graph_command_without_facade_feedback() {
    let project_root = temp_project_root("asp-hook-root-graph-command");
    let runtime = runtime_for_project(&project_root);

    let decision = classify_hook(
        &runtime,
        "codex",
        "pre-tool",
        &json!({
            "hook_event_name": "PreToolUse",
            "session_id": "session-graph",
            "transcript_path": "transcript-graph.jsonl",
            "tool_name": "Bash",
            "tool_input": {
                "command": "asp graph render --packet - --view seeds"
            }
        }),
    );

    assert_eq!(decision.decision, DecisionKind::Allow);
    assert!(!decision.fields.contains_key("hookFeedback"));
    assert!(!decision.fields.contains_key("invalidFacade"));
    let _ = fs::remove_dir_all(project_root);
}

#[test]
fn pre_tool_allows_root_plugin_install_command_without_facade_feedback() {
    let project_root = temp_project_root("asp-hook-root-plugin-command");
    let runtime = runtime_for_project(&project_root);

    let decision = classify_hook(
        &runtime,
        "codex",
        "pre-tool",
        &json!({
            "hook_event_name": "PreToolUse",
            "session_id": "session-plugin",
            "transcript_path": "transcript-plugin.jsonl",
            "tool_name": "Bash",
            "tool_input": {
                "command": "target/debug/asp plugin install codex ."
            }
        }),
    );

    assert_eq!(decision.decision, DecisionKind::Allow);
    assert!(!decision.fields.contains_key("hookFeedback"));
    assert!(!decision.fields.contains_key("invalidFacade"));
    let _ = fs::remove_dir_all(project_root);
}
