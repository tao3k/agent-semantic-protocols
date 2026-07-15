use agent_semantic_hook::{
    ActionPolicy, DecisionKind, DecisionRouteKind, ReasonKind, classify_hook,
};
use serde_json::json;

use super::support::{assert_allowed, polyglot_registry, rust_registry};

use crate::classifier::raw_search_policy::support::assert_raw_search_denied;

#[test]
fn workspace_wide_raw_search_without_language_selector_is_denied() {
    for command in [
        "rg -n WorkflowExecution",
        "rg -n WorkflowExecution src",
        "fd WorkflowExecution",
        "fd -t f WorkflowExecution src",
    ] {
        assert_raw_search_denied(command, "ts-harness");
    }
}

#[test]
fn direct_read_of_real_source_directory_is_denied() {
    let source_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let decision = classify_hook(
        &polyglot_registry(),
        "codex",
        "pre-tool",
        &json!({
            "tool_name": "Read",
            "tool_input": {"path": source_dir}
        }),
    );

    assert_eq!(decision.decision, DecisionKind::Deny);
    assert_eq!(decision.reason_kind, ReasonKind::SourceDirectoryEnumeration);
}

#[test]
fn direct_read_of_exact_source_file_is_denied_and_routes_to_provider_query() {
    let decision = classify_hook(
        &polyglot_registry(),
        "codex",
        "pre-tool",
        &json!({
            "tool_name": "Read",
            "tool_input": {"path": "src/cli/agent-hooks.ts"}
        }),
    );

    assert_eq!(decision.decision, DecisionKind::Deny);
    assert_eq!(decision.reason_kind, ReasonKind::DirectSourceRead);
    assert_eq!(decision.language_ids, vec!["typescript".to_string()]);
    assert_eq!(decision.routes[0].kind, DecisionRouteKind::Owner);
    assert_eq!(
        decision.routes[0].argv,
        vec![
            "asp",
            "typescript",
            "search",
            "owner",
            "src/cli/agent-hooks.ts",
            "items",
            "--workspace",
            ".",
            "--view",
            "seeds",
        ]
    );
}

#[test]
fn direct_read_of_exact_rust_source_file_is_denied() {
    let decision = classify_hook(
        &polyglot_registry(),
        "codex",
        "pre-tool",
        &json!({
            "tool_name": "Read",
            "tool_input": {"path": "crates/agent-semantic-hook/src/hook_config/core.rs"}
        }),
    );

    assert_eq!(decision.decision, DecisionKind::Deny);
    assert_eq!(decision.reason_kind, ReasonKind::DirectSourceRead);
    assert_eq!(decision.language_ids, vec!["rust".to_string()]);
    assert_eq!(decision.routes[0].kind, DecisionRouteKind::Owner);
}

#[test]
fn exact_shell_source_dump_is_denied() {
    for (command, reason_kind) in [
        ("cat src/cli/agent-hooks.ts", ReasonKind::BulkSourceDump),
        (
            "sed -n '1,40p' src/cli/agent-hooks.ts",
            ReasonKind::BulkSourceDump,
        ),
        (
            "head -n 40 src/cli/agent-hooks.ts",
            ReasonKind::BulkSourceDump,
        ),
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
        assert_eq!(decision.reason_kind, reason_kind, "{command}");
        assert_eq!(decision.routes[0].kind, DecisionRouteKind::Owner);
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
        "fd -t f shell -c \"echo a.rs\"",
    ] {
        let decision = classify_hook(
            &rust_registry(),
            "codex",
            "pre-tool",
            &json!({
                "tool_name": "functions.exec_command",
                "tool_input": { "cmd": command }
            }),
        );

        assert_eq!(decision.decision, DecisionKind::Deny, "{command}");
        assert_eq!(decision.reason_kind, ReasonKind::RawBroadSearch);
        assert_eq!(decision.routes[0].binary, "asp");
        assert_eq!(decision.routes[0].provider_id, "rs-harness");
    }
}

#[test]
fn raw_search_globs_without_suffix_respect_provider_source_roots() {
    assert_allowed("rg --files -g 'docs/**'");
    assert_raw_search_denied("rg --files -g 'crates/**'", "rs-harness");
    assert_raw_search_denied("rg --files -g '**/*'", "ts-harness");
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
            "tool_input": {"cmd": "rg -n WorkflowExecution src | asp typescript search ingest owner tests --workspace . --view seeds"}
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
fn git_diff_patch_review_commands_are_allowed() {
    for command in [
        "git diff -- languages/python-lang-project-harness/src/python_lang_project_harness/_python_compact.py",
        "git diff --no-index /dev/null languages/python-lang-project-harness/src/python_lang_project_harness/_python_compact.py",
        "git -C languages/python-lang-project-harness diff -- src/python_lang_project_harness/_python_compact.py",
    ] {
        assert_allowed(command);
    }
}

#[test]
fn broad_raw_search_routes_to_lexical_frontier_when_supported() {
    let decision = classify_hook(
        &polyglot_registry(),
        "codex",
        "pre-tool",
        &json!({
            "tool_name": "functions.exec_command",
            "tool_input": { "cmd": "rg -n WorkflowExecution src/cli/protocol.ts" }
        }),
    );

    assert_eq!(decision.decision, DecisionKind::Deny);
    assert_eq!(decision.reason_kind, ReasonKind::RawBroadSearch);
    assert_eq!(decision.language_ids, vec!["typescript".to_string()]);
    assert_eq!(decision.routes[0].kind, DecisionRouteKind::Lexical);
    assert!(
        decision.routes[0]
            .argv
            .windows(2)
            .any(|window| window[0] == "search" && window[1] == "lexical")
    );
    assert!(
        !decision.routes[0]
            .argv
            .iter()
            .any(|arg| arg == "direct-source-read")
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
fn action_policy_allows_raw_search_and_explicit_reads() {
    let mut registry = crate::classifier::registry();
    registry.providers[0].policy.raw_source_search = ActionPolicy::Allow;
    registry.providers[0].policy.direct_source_read = ActionPolicy::Allow;
    registry.providers[0].policy.bulk_source_dump = ActionPolicy::Allow;

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
    let dump_decision = classify_hook(
        &registry,
        "codex",
        "pre-tool",
        &json!({
            "tool_name": "functions.exec_command",
            "tool_input": {"cmd": "sed -n '1,40p' src/cli/agent-hooks.ts"}
        }),
    );

    assert_eq!(search_decision.decision, DecisionKind::Allow);
    assert_eq!(read_decision.decision, DecisionKind::Allow);
    assert_eq!(read_decision.reason_kind, ReasonKind::None);
    assert_eq!(dump_decision.decision, DecisionKind::Allow);
    assert_eq!(dump_decision.reason_kind, ReasonKind::None);
}
