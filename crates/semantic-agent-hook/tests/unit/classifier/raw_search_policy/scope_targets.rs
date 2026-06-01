use semantic_agent_hook::{
    DecisionKind, DecisionRouteKind, ReasonKind, classify_hook, parse_profiles,
};
use serde_json::json;

use crate::classifier::registry_value;

use super::support::{assert_allowed, polyglot_registry, rust_registry};

#[test]
fn workspace_wide_raw_search_without_scope_is_denied_for_all_profiles() {
    for command in [
        "rg -n WorkflowExecution",
        "rg --files .",
        "fd WorkflowExecution",
        "git grep WorkflowExecution",
        "git ls-files",
    ] {
        let decision = classify_hook(
            &polyglot_registry(),
            "codex",
            "pre-tool",
            &json!({
                "tool_name": "functions.exec_command",
                "tool_input": {"cmd": command}
            }),
        );

        assert_eq!(decision.decision, DecisionKind::Deny, "{command}");
        assert_eq!(decision.reason_kind, ReasonKind::RawBroadSearch);
        assert_eq!(
            decision.language_ids,
            vec![
                "typescript".to_string(),
                "rust".to_string(),
                "python".to_string()
            ],
            "{command}"
        );
    }
}

#[test]
fn broad_rust_raw_search_with_filters_routes_to_ingest() {
    for command in [
        "rg -n HookToolName -t rust crates/semantic-agent-hook/src",
        "rg -n HookToolName -g '*.rs' crates/semantic-agent-hook/src",
        "rg -n HookToolName -t rust crates/semantic-agent-hook/src/lib.rs crates/semantic-agent-hook/src",
        "fd -e rs lib crates",
        "find crates -name '*.rs' -print",
    ] {
        let decision = classify_hook(
            &rust_registry(),
            "codex",
            "pre-tool",
            &json!({
                "tool_name": "functions.exec_command",
                "tool_input": {"cmd": command}
            }),
        );

        assert_eq!(decision.decision, DecisionKind::Deny, "{command}");
        assert_eq!(decision.reason_kind, ReasonKind::RawBroadSearch);
        assert_eq!(decision.routes[0].binary, "rs-harness");
    }
}

#[test]
fn raw_search_globs_without_suffix_are_not_language_evidence() {
    for command in [
        "rg --files -g 'docs/**'",
        "rg --files -g 'crates/**'",
        "rg --files -g '**/*'",
    ] {
        assert_allowed(command);
    }
}

#[test]
fn global_suffix_glob_raw_search_targets_matching_language_profiles() {
    let decision = classify_hook(
        &polyglot_registry(),
        "codex",
        "pre-tool",
        &json!({
            "tool_name": "functions.exec_command",
            "tool_input": {"cmd": "rg --files -g '**/*.{rs,py}'"}
        }),
    );

    assert_eq!(decision.decision, DecisionKind::Deny);
    assert_eq!(decision.reason_kind, ReasonKind::RawBroadSearch);
    assert_eq!(
        decision.language_ids,
        vec!["rust".to_string(), "python".to_string()]
    );
}

#[test]
fn raw_search_piped_to_ingest_is_allowed() {
    let decision = classify_hook(
        &polyglot_registry(),
        "codex",
        "pre-tool",
        &json!({
            "tool_name": "functions.exec_command",
            "tool_input": {"cmd": "rg -n WorkflowExecution src | ts-harness search ingest owner tests --view seeds ."}
        }),
    );

    assert_eq!(decision.decision, DecisionKind::Allow);
}

#[test]
fn non_search_git_commands_are_allowed() {
    for command in ["git status .", "git diff --stat", "git log --oneline -5"] {
        assert_allowed(command);
    }
}

#[test]
fn broad_raw_search_routes_to_ingest() {
    let decision = classify_hook(
        &polyglot_registry(),
        "codex",
        "pre-tool",
        &json!({
            "tool_name": "functions.exec_command",
            "tool_input": {"cmd": "rg -n WorkflowExecution src tests"}
        }),
    );

    assert_eq!(decision.decision, DecisionKind::Deny);
    assert_eq!(decision.reason_kind, ReasonKind::RawBroadSearch);
    assert_eq!(decision.routes[0].kind, DecisionRouteKind::Ingest);
}

#[test]
fn action_policy_can_allow_raw_search_without_allowing_direct_reads() {
    let mut value = registry_value();
    value["profiles"][0]["policy"]["rawSourceSearch"] = json!("allow");
    let registry = parse_profiles(&value.to_string()).unwrap();

    let search_decision = classify_hook(
        &registry,
        "codex",
        "pre-tool",
        &json!({
            "tool_name": "functions.exec_command",
            "tool_input": {"cmd": "rg -n WorkflowExecution src"}
        }),
    );
    let read_decision = classify_hook(
        &registry,
        "codex",
        "pre-tool",
        &json!({
            "tool_name": "Read",
            "tool_input": {"path": "src/cli/agent-hooks.ts"}
        }),
    );

    assert_eq!(search_decision.decision, DecisionKind::Allow);
    assert_eq!(read_decision.decision, DecisionKind::Deny);
    assert_eq!(read_decision.reason_kind, ReasonKind::DirectSourceRead);
}
