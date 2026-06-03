use semantic_agent_hook::{DecisionKind, DecisionRouteKind, ReasonKind, classify_hook};
use serde_json::json;

use super::support::rust_harness_activation;

#[test]
fn root_owned_rust_activation_tracks_rust_harness_default_scope() {
    let config = rust_lang_project_harness::default_rust_harness_config();
    let runtime = rust_harness_activation();
    let provider = &runtime.providers[0];

    for root in config
        .source_dir_names
        .iter()
        .chain(config.test_dir_names.iter())
    {
        assert!(
            provider.source_roots.contains(root),
            "rust activation is missing rust harness source root {root}"
        );
    }
    for ignored in config.ignored_dir_names {
        assert!(
            provider.ignored_path_prefixes.contains(&ignored),
            "rust activation is missing rust harness ignored prefix {ignored}"
        );
    }
}

#[test]
fn root_owned_rust_activation_uses_shared_hook_schema() {
    let runtime = rust_harness_activation();

    assert_eq!(runtime.project_root, ".");
    assert_eq!(runtime.providers[0].source_roots[0], "src");
}

#[test]
fn rust_harness_activation_uses_provider_identity() {
    let runtime = rust_harness_activation();
    assert_eq!(runtime.providers.len(), 1);
    let provider = &runtime.providers[0];
    assert_eq!(provider.language_id, "rust");
    assert_eq!(provider.provider_id, "rs-harness");
    assert_eq!(provider.binary, "rs-harness");
    assert!(provider.source_roots.iter().any(|root| root == "src"));
    assert!(
        provider
            .source_extensions
            .iter()
            .any(|extension| extension == ".rs")
    );
    let guide = provider.routes.guide.as_ref().expect("guide command");
    assert_eq!(
        guide.argv,
        ["rs-harness", "agent", "guide", "{projectRoot}"]
    );
}

#[test]
fn rust_harness_activation_routes_direct_reads_to_provider_query() {
    let decision = classify_hook(
        &rust_harness_activation(),
        "codex",
        "pre-tool",
        &json!({
            "tool_name": "Read",
            "tool_input": {"path": "src/lib.rs"}
        }),
    );

    assert_eq!(decision.decision, DecisionKind::Deny);
    assert_eq!(decision.reason_kind, ReasonKind::DirectSourceRead);
    assert_eq!(decision.routes[0].kind, DecisionRouteKind::Query);
    assert_eq!(decision.routes[0].provider_id, "rs-harness");
    assert_eq!(
        decision.routes[0].argv,
        [
            "rs-harness",
            "query",
            "--from-hook",
            "direct-source-read",
            "--selector",
            "src/lib.rs",
            "--code",
            "."
        ]
    );
}

#[test]
fn rust_harness_activation_routes_raw_root_search_to_hook_query() {
    let decision = classify_hook(
        &rust_harness_activation(),
        "codex",
        "pre-tool",
        &json!({
            "tool_name": "functions.exec_command",
            "tool_input": {"cmd": "rg -n \"HookDecision\" ."}
        }),
    );

    assert_eq!(decision.decision, DecisionKind::Deny);
    assert_eq!(decision.reason_kind, ReasonKind::RawBroadSearch);
    assert_eq!(decision.routes[0].kind, DecisionRouteKind::Query);
    assert_eq!(
        decision.routes[0].argv,
        [
            "rs-harness",
            "search",
            "query",
            "--from-hook",
            "direct-source-read",
            "--selector",
            "**/*.rs",
            "--term",
            "HookDecision",
            "--surface",
            "owner,tests",
            "--view",
            "seeds",
            "."
        ]
    );
}
