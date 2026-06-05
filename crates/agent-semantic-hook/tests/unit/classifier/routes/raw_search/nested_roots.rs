use agent_semantic_hook::{DecisionKind, DecisionRouteKind, ReasonKind, classify_hook};
use serde_json::json;

use super::support::registry;

#[test]
fn codex_raw_search_nested_provider_source_root_routes_as_raw_search() {
    let mut registry = registry();
    let rust = registry
        .providers
        .iter_mut()
        .find(|provider| provider.language_id == "rust")
        .unwrap();
    rust.package_roots = vec!["languages/rust-lang-project-harness".to_string()];

    let decision = classify_hook(
        &registry,
        "codex",
        "pre-tool",
        &json!({
            "toolName": "command_execution",
            "tool_name": "command_execution",
            "toolInput": {
                "commandActions": [{
                    "type": "search",
                    "command": "rg -n SyntaxQueryRow languages/rust-lang-project-harness/src",
                    "query": "SyntaxQueryRow",
                    "path": "languages/rust-lang-project-harness/src"
                }]
            },
            "tool_input": {
                "commandActions": [{
                    "type": "search",
                    "command": "rg -n SyntaxQueryRow languages/rust-lang-project-harness/src",
                    "query": "SyntaxQueryRow",
                    "path": "languages/rust-lang-project-harness/src"
                }]
            }
        }),
    );

    assert_eq!(decision.decision, DecisionKind::Deny);
    assert_eq!(decision.reason_kind, ReasonKind::RawBroadSearch);
    assert_eq!(decision.language_ids, vec!["rust"]);
    assert_eq!(decision.routes[0].kind, DecisionRouteKind::Query);
    assert!(
        decision.routes[0]
            .argv
            .iter()
            .any(|arg| arg == "SyntaxQueryRow")
    );
}
