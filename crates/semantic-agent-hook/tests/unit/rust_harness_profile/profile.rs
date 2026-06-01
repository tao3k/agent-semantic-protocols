use semantic_agent_hook::{DecisionKind, DecisionRouteKind, ReasonKind, StdinMode, classify_hook};
use serde_json::json;

use super::support::rust_harness_profile_registry;

#[test]
fn root_owned_rust_profile_tracks_rust_harness_default_scope() {
    let config = rust_lang_project_harness::default_rust_harness_config();
    let registry = rust_harness_profile_registry();
    let profile = &registry.profiles[0];

    for root in config
        .source_dir_names
        .iter()
        .chain(config.test_dir_names.iter())
    {
        assert!(
            profile.source_roots.contains(root),
            "rust profile is missing rust harness source root {root}"
        );
    }
    for ignored in config.ignored_dir_names {
        assert!(
            profile.ignored_path_prefixes.contains(&ignored),
            "rust profile is missing rust harness ignored prefix {ignored}"
        );
    }
}

#[test]
fn root_owned_rust_profile_uses_shared_hook_schema() {
    let registry = rust_harness_profile_registry();

    assert_eq!(
        registry.schema_id,
        semantic_agent_hook::PROFILE_REGISTRY_SCHEMA_ID
    );
    assert_eq!(registry.protocol_id, semantic_agent_hook::HOOK_PROTOCOL_ID);
    assert_eq!(registry.profiles[0].source_roots[0], "src");
}

#[test]
fn rust_harness_profile_uses_provider_identity() {
    let registry = rust_harness_profile_registry();
    assert_eq!(registry.profiles.len(), 1);
    let profile = &registry.profiles[0];
    assert_eq!(profile.language_id, "rust");
    assert_eq!(profile.provider_id, "rs-harness");
    assert_eq!(profile.binary, "rs-harness");
    assert!(profile.source_roots.iter().any(|root| root == "src"));
    assert!(
        profile
            .source_extensions
            .iter()
            .any(|extension| extension == ".rs")
    );
}

#[test]
fn rust_harness_profile_routes_direct_reads_to_owner_search() {
    let decision = classify_hook(
        &rust_harness_profile_registry(),
        "codex",
        "pre-tool",
        &json!({
            "tool_name": "Read",
            "tool_input": {"path": "src/lib.rs"}
        }),
    );

    assert_eq!(decision.decision, DecisionKind::Deny);
    assert_eq!(decision.reason_kind, ReasonKind::DirectSourceRead);
    assert_eq!(
        decision.routes[0].argv,
        [
            "rs-harness",
            "search",
            "owner",
            "src/lib.rs",
            "items",
            "--view",
            "seeds",
            "."
        ]
    );
}

#[test]
fn rust_harness_profile_routes_raw_root_search_to_ingest() {
    let decision = classify_hook(
        &rust_harness_profile_registry(),
        "codex",
        "pre-tool",
        &json!({
            "tool_name": "functions.exec_command",
            "tool_input": {"cmd": "rg -n \"HookDecision\" ."}
        }),
    );

    assert_eq!(decision.decision, DecisionKind::Deny);
    assert_eq!(decision.reason_kind, ReasonKind::RawBroadSearch);
    assert_eq!(decision.routes[0].kind, DecisionRouteKind::Ingest);
    assert_eq!(
        decision.routes[0].stdin_mode,
        Some(StdinMode::PipeCandidates)
    );
}
