use agent_semantic_hook::{
    ActionPolicy, DecisionKind, DecisionRouteKind, ReasonKind, classify_hook,
};
use serde_json::json;

use super::support::{assert_allowed, polyglot_registry, rust_registry};

#[test]
fn workspace_wide_raw_search_without_scope_is_denied_for_all_providers() {
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
        "rg -n HookToolName -t rust crates/agent-semantic-hook/src",
        "rg -n HookToolName -g '*.rs' crates/agent-semantic-hook/src",
        "rg -n HookToolName -t rust crates/agent-semantic-hook/src/lib.rs crates/agent-semantic-hook/src",
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
        assert_eq!(decision.routes[0].binary, "asp");
        assert_eq!(decision.routes[0].provider_id, "rs-harness");
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
fn non_source_scoped_raw_search_stays_allowed() {
    for command in [
        "rg -n WorkflowExecution docs README.md",
        "rg -n WorkflowExecution -g '*.md' docs",
        "fd WorkflowExecution docs",
        "fd README docs",
        "find docs -name '*.md' -print",
        "git grep WorkflowExecution -- docs README.md",
    ] {
        assert_allowed(command);
    }
}

#[test]
fn global_suffix_glob_raw_search_targets_matching_language_providers() {
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
            "tool_input": {"cmd": "rg -n WorkflowExecution src | asp typescript search ingest owner tests --view seeds ."}
        }),
    );

    assert_eq!(decision.decision, DecisionKind::Allow);
}

#[test]
fn non_search_git_commands_are_allowed() {
    for command in [
        "git status .",
        "git diff --stat",
        "git diff --name-only",
        "git diff --name-status",
        "git log --oneline -5",
    ] {
        assert_allowed(command);
    }
}

#[test]
fn git_diff_source_output_routes_to_language_provider() {
    for command in [
        "git diff -- languages/python-lang-project-harness/src/python_lang_project_harness/_python_compact.py",
        "git diff --no-index /dev/null languages/python-lang-project-harness/src/python_lang_project_harness/_python_compact.py",
        "git -C languages/python-lang-project-harness diff -- src/python_lang_project_harness/_python_compact.py",
    ] {
        assert_bulk_source_dump_denied(command, "py-harness");
    }
}

#[test]
fn broad_raw_search_routes_to_provider_query_when_supported() {
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
    assert_eq!(decision.routes[0].kind, DecisionRouteKind::Query);
    assert_eq!(
        decision.routes[0].argv,
        [
            "asp",
            "typescript",
            "query",
            "--from-hook",
            "direct-source-read",
            "--selector",
            "**/*.{cjs,cts,js,jsx,mjs,mts,ts,tsx}",
            "--term",
            "WorkflowExecution",
            "--surface",
            "owners,tests",
            "--view",
            "seeds",
            ".",
        ]
    );
}

#[test]
fn fd_extension_source_path_listing_routes_to_ingest() {
    let decision = classify_hook(
        &polyglot_registry(),
        "codex",
        "pre-tool",
        &json!({
            "tool_name": "functions.exec_command",
            "tool_input": {"cmd": "fd -e ts src"}
        }),
    );
    assert_eq!(decision.decision, DecisionKind::Deny);
    assert_eq!(decision.reason_kind, ReasonKind::RawBroadSearch);
    assert_eq!(decision.routes[0].kind, DecisionRouteKind::Ingest);
    assert_eq!(decision.routes[0].provider_id, "ts-harness");
}

#[test]
fn action_policy_can_allow_raw_search_without_allowing_direct_reads() {
    let mut registry = crate::classifier::registry();
    registry.providers[0].policy.raw_source_search = ActionPolicy::Allow;

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
use crate::classifier::raw_search_policy::support::assert_bulk_source_dump_denied;
