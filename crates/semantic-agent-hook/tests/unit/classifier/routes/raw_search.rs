use semantic_agent_hook::{DecisionKind, DecisionRouteKind, ReasonKind, classify_hook};
use serde_json::json;

use crate::classifier::registry;

#[test]
fn broad_raw_search_routes_to_hook_query() {
    let decision = classify_hook(
        &registry(),
        "codex",
        "pre-tool",
        &json!({
            "tool_name": "functions.exec_command",
            "tool_input": {"cmd": "rg -n WorkflowExecution src tests"}
        }),
    );

    assert_eq!(decision.decision, DecisionKind::Deny);
    assert_eq!(decision.reason_kind, ReasonKind::RawBroadSearch);
    assert_eq!(decision.routes[0].kind, DecisionRouteKind::Text);
    assert_eq!(
        decision.routes[0].argv,
        [
            "ts-harness",
            "search",
            "query",
            "--from-hook",
            "direct-source-read",
            "--selector",
            "**/*.{ts,tsx}",
            "--term",
            "WorkflowExecution",
            "--surface",
            "owner,tests",
            "--view",
            "seeds",
            "."
        ]
    );
    assert_eq!(
        decision.message,
        "raw-broad-search denied; provider guide: ts-harness => ts-harness agent guide ."
    );
}

#[test]
fn provider_output_filtering_is_allowed() {
    for command in [
        "ts-harness --help | rg -- '--code|query <owner-path>'",
        "ts-harness agent guide . | rg -- '--code'",
        "py-harness --help | rg -- '--code|query <owner-path>'",
        "rs-harness agent guide . | rg -- '--code'",
    ] {
        let decision = classify_hook(
            &super::registry_with_rust_and_python(),
            "codex",
            "pre-tool",
            &json!({
                "tool_name": "functions.exec_command",
                "tool_input": {"cmd": command}
            }),
        );
        assert_eq!(decision.decision, DecisionKind::Allow, "{command}");
        assert_eq!(decision.reason_kind, ReasonKind::None, "{command}");
    }
}

#[test]
fn workspace_provider_output_filtering_requires_full_command_prefix() {
    let allowed = "julia --project=languages/JuliaLangProjectHarness.jl languages/JuliaLangProjectHarness.jl/bin/julia-project-harness.jl agent guide . | rg -- 'search owner'";
    let allowed_decision = classify_hook(
        &super::registry_with_workspace_julia(),
        "codex",
        "pre-tool",
        &json!({
            "tool_name": "functions.exec_command",
            "tool_input": {"cmd": allowed}
        }),
    );
    assert_eq!(allowed_decision.decision, DecisionKind::Allow);
    assert_eq!(allowed_decision.reason_kind, ReasonKind::None);

    let denied = "julia -e 'println(\"raw\")' | rg -- 'raw'";
    let denied_decision = classify_hook(
        &super::registry_with_workspace_julia(),
        "codex",
        "pre-tool",
        &json!({
            "tool_name": "functions.exec_command",
            "tool_input": {"cmd": denied}
        }),
    );
    assert_eq!(denied_decision.decision, DecisionKind::Deny);
    assert_eq!(denied_decision.reason_kind, ReasonKind::RawBroadSearch);
}

#[test]
fn raw_file_listing_without_query_keeps_ingest_route() {
    let decision = classify_hook(
        &registry(),
        "codex",
        "pre-tool",
        &json!({
            "tool_name": "functions.exec_command",
            "tool_input": {"cmd": "rg --files -g '*.ts'"}
        }),
    );

    assert_eq!(decision.decision, DecisionKind::Deny);
    assert_eq!(decision.reason_kind, ReasonKind::RawBroadSearch);
    assert_eq!(decision.routes[0].kind, DecisionRouteKind::Ingest);
}

#[test]
fn raw_regex_alternation_routes_to_query_terms() {
    let decision = classify_hook(
        &registry(),
        "codex",
        "pre-tool",
        &json!({
            "tool_name": "functions.exec_command",
            "tool_input": {"cmd": "rg -n 'parseSearchArgs|querySets|buildSemanticSearchPacket' -g '**/*.{ts,tsx,js}'"}
        }),
    );

    assert_eq!(decision.decision, DecisionKind::Deny);
    assert_eq!(decision.reason_kind, ReasonKind::RawBroadSearch);
    assert_eq!(decision.routes[0].kind, DecisionRouteKind::Text);
    assert_eq!(
        decision.routes[0].argv,
        [
            "ts-harness",
            "search",
            "query",
            "--from-hook",
            "direct-source-read",
            "--selector",
            "**/*.{ts,tsx}",
            "--term",
            "parseSearchArgs",
            "--term",
            "querySets",
            "--term",
            "buildSemanticSearchPacket",
            "--surface",
            "owner,tests",
            "--view",
            "seeds",
            "."
        ]
    );
}

#[test]
fn raw_search_pattern_flags_route_to_query_terms() {
    for command in [
        "rg -n -e parseSearchArgs -e querySets -g '*.ts' src",
        "grep -R -n -e parseSearchArgs --include='*.ts' src",
        "git grep -n -e parseSearchArgs -- '*.ts'",
    ] {
        let decision = classify_hook(
            &registry(),
            "codex",
            "pre-tool",
            &json!({
                "tool_name": "functions.exec_command",
                "tool_input": {"cmd": command}
            }),
        );

        assert_eq!(decision.decision, DecisionKind::Deny, "{command}");
        assert_eq!(decision.reason_kind, ReasonKind::RawBroadSearch);
        assert_eq!(decision.routes[0].kind, DecisionRouteKind::Text);
        assert!(
            decision.routes[0]
                .argv
                .windows(2)
                .any(|pair| pair == ["--term", "parseSearchArgs"]),
            "{command}"
        );
    }
}

#[test]
fn fd_filename_query_routes_to_hook_query() {
    let decision = classify_hook(
        &registry(),
        "codex",
        "pre-tool",
        &json!({
            "tool_name": "functions.exec_command",
            "tool_input": {"cmd": "fd parseSearchArgs -e ts"}
        }),
    );

    assert_eq!(decision.decision, DecisionKind::Deny);
    assert_eq!(decision.reason_kind, ReasonKind::RawBroadSearch);
    assert_eq!(decision.routes[0].kind, DecisionRouteKind::Text);
    assert!(
        decision.routes[0]
            .argv
            .windows(2)
            .any(|pair| pair == ["--term", "parseSearchArgs"])
    );
}

#[test]
fn find_extension_listing_without_name_query_keeps_ingest_route() {
    let decision = classify_hook(
        &registry(),
        "codex",
        "pre-tool",
        &json!({
            "tool_name": "functions.exec_command",
            "tool_input": {"cmd": "find . -name '*.ts'"}
        }),
    );

    assert_eq!(decision.decision, DecisionKind::Deny);
    assert_eq!(decision.reason_kind, ReasonKind::RawBroadSearch);
    assert_eq!(decision.routes[0].kind, DecisionRouteKind::Ingest);
}
