use agent_semantic_hook::{DecisionKind, DecisionRouteKind, ReasonKind, classify_hook};
use serde_json::json;

use super::support::{root_owned_rust_activation_json, rust_harness_activation};

#[test]
fn root_owned_rust_activation_tracks_rust_harness_default_scope() {
    let runtime = rust_harness_activation();
    let provider = &runtime.providers[0];

    assert_eq!(provider.package_roots, ["."]);
    assert!(
        provider
            .source_extensions
            .iter()
            .any(|value| value == ".rs")
    );
    assert!(
        provider
            .config_files
            .iter()
            .any(|value| value == "Cargo.toml")
    );
}

#[test]
fn root_owned_rust_activation_uses_shared_hook_schema() {
    let runtime = rust_harness_activation();

    assert_eq!(runtime.project_root, ".");
    assert_eq!(runtime.providers[0].package_roots[0], ".");
}

#[test]
fn root_owned_rust_activation_is_generated_by_asp() {
    let activation: serde_json::Value =
        serde_json::from_str(&root_owned_rust_activation_json()).expect("parse activation fixture");

    assert_eq!(activation["generatedBy"]["runtime"], "asp");
}

#[test]
fn rust_harness_activation_uses_provider_identity() {
    let runtime = rust_harness_activation();
    assert_eq!(runtime.providers.len(), 1);
    let provider = &runtime.providers[0];
    assert_eq!(provider.language_id, "rust");
    assert_eq!(provider.provider_id, "asp-rust");
    assert_eq!(provider.binary, "asp-rust");
    assert_eq!(provider.package_roots, ["."]);
    assert!(
        provider
            .source_extensions
            .iter()
            .any(|extension| extension == ".rs")
    );
    let guide = provider.routes.guide.as_ref().expect("guide command");
    assert_eq!(guide.argv, ["asp-rust", "guide", "{workspace}"]);
}

#[test]
fn rust_harness_activation_routes_explicit_reads_to_owner_frontier() {
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
    assert_eq!(
        decision.reason_kind,
        ReasonKind::RegisteredSourceRouteRequired
    );
    assert_eq!(decision.language_ids, ["rust"]);
    assert_eq!(decision.routes[0].kind, DecisionRouteKind::Playbook);
    assert_eq!(decision.routes[0].provider_id, "asp-rust");
}

#[test]
fn rust_harness_activation_routes_source_glob_search_to_lexical_frontier() {
    let decision = classify_hook(
        &rust_harness_activation(),
        "codex",
        "pre-tool",
        &json!({
            "tool_name": "functions.exec_command",
            "tool_input": { "cmd": "rg -n -g '*.rs' HookDecision ." }
        }),
    );

    assert_eq!(decision.decision, DecisionKind::Deny);
    assert_eq!(decision.reason_kind, ReasonKind::RegisteredSourceRouteRequired);
    assert_eq!(
        decision
            .fields
            .get("configRuleId")
            .and_then(|value| value.as_str()),
        Some("route-read-to-asp-languages")
    );
    assert!(decision.message.contains("asp session"));
    assert!(!decision.message.contains("ASP Explore"));
}
