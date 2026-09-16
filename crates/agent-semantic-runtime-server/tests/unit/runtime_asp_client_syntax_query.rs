// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use agent_semantic_client_protocol::EnhancedQueryCapabilityTable;

use super::{
    producer_matches_optional_calibration, project_selected_fields, resident_pattern_capture,
    resident_regex_programs, validate_resident_query_plan,
};

const GENERATION_DIGEST: &str =
    "blake3-256:1111111111111111111111111111111111111111111111111111111111111111";

fn rust_capability() -> EnhancedQueryCapabilityTable {
    serde_json::from_str(include_str!(
        "../../../../languages/asp-rust/tree-sitter/tree-sitter-rust/enhanced-query-capabilities.v1.json"
    ))
    .expect("Rust enhanced Query capability table")
}

fn provider() -> agent_semantic_search::WorkspaceSearchProvider {
    agent_semantic_search::WorkspaceSearchProvider {
        language_id: "rust".to_owned(),
        provider_id: "asp-rust".to_owned(),
        source_extensions: vec!["rs".to_owned()],
        search_supported: true,
        producer_axes: vec![agent_semantic_search::WorkspaceSearchProducerAxis::Language],
        enhanced_query_capability: Some(rust_capability()),
    }
}

fn selector(name: &str) -> agent_semantic_search::NativeSyntaxSelector {
    agent_semantic_search::NativeSyntaxSelector {
        selector: format!("rust://src/lib.rs#item/function/{name}"),
        byte_start: 1,
        byte_end: 2,
        query_keys: vec![name.to_owned()],
        derived_projection_digest: format!("blake3-256:{}", "a".repeat(64)),
    }
}

#[test]
fn absent_language_calibration_does_not_reject_an_explicit_syntax_producer() {
    assert!(producer_matches_optional_calibration(
        &Default::default(),
        "rust"
    ));
    assert!(producer_matches_optional_calibration(
        &["rust"].into_iter().collect(),
        "rust"
    ));
    assert!(!producer_matches_optional_calibration(
        &["python"].into_iter().collect(),
        "rust"
    ));
}

#[test]
fn resident_regex_uses_the_precompiled_program_without_a_parser() {
    let plan = agent_semantic_tree_sitter::compile_resident_syntax_plan(
        r#"(_) @item (#asp-match? @item "name" "^RuntimeQueryGeneration$")"#,
        GENERATION_DIGEST,
        &rust_capability(),
    )
    .expect("compile resident query plan");
    let provider = provider();
    validate_resident_query_plan(&plan, GENERATION_DIGEST, &provider)
        .unwrap_or_else(|_| panic!("admit resident plan"));
    let programs =
        resident_regex_programs(&plan).unwrap_or_else(|_| panic!("decode admitted regex program"));
    let matching = selector("RuntimeQueryGeneration");
    let matching_canonical =
        agent_semantic_content_identity::CanonicalItemSelector::parse(matching.selector.clone())
            .expect("canonical matching selector");
    let other = selector("Other");
    let other_canonical =
        agent_semantic_content_identity::CanonicalItemSelector::parse(other.selector.clone())
            .expect("canonical other selector");

    assert_eq!(
        resident_pattern_capture(
            plan.patterns.first().expect("one pattern"),
            &matching_canonical,
            &matching,
            &programs,
        )
        .unwrap_or_else(|_| panic!("evaluate resident predicate")),
        Some("item".to_owned())
    );
    assert_eq!(
        resident_pattern_capture(
            plan.patterns.first().expect("one pattern"),
            &other_canonical,
            &other,
            &programs,
        )
        .unwrap_or_else(|_| panic!("evaluate resident predicate")),
        None
    );
}

#[test]
fn selected_runtime_fields_are_returned_from_the_same_matched_item() {
    let plan = agent_semantic_tree_sitter::compile_resident_syntax_plan(
        r#"(function_item) @item
            (#asp-select! @item "kind" "name" "selector")"#,
        GENERATION_DIGEST,
        &rust_capability(),
    )
    .expect("compile selected fields plan");
    let selector = selector("run");
    let canonical =
        agent_semantic_content_identity::CanonicalItemSelector::parse(selector.selector.clone())
            .expect("canonical selector");
    let selected = project_selected_fields(&plan, &canonical, &selector)
        .unwrap_or_else(|_| panic!("project selected fields"));

    assert_eq!(selected.kind.as_deref(), Some("function"));
    assert_eq!(selected.name.as_deref(), Some("run"));
    assert_eq!(
        selected.selector.as_deref(),
        Some(selector.selector.as_str())
    );
    assert!(selected.byte_range.is_none());
    assert!(selected.query_keys.is_empty());
}

#[test]
fn resident_query_rejects_generation_tampering() {
    let plan = agent_semantic_tree_sitter::compile_resident_syntax_plan(
        "(function_item) @item",
        GENERATION_DIGEST,
        &rust_capability(),
    )
    .expect("compile resident query plan");
    let error = validate_resident_query_plan(
        &plan,
        "blake3-256:2222222222222222222222222222222222222222222222222222222222222222",
        &provider(),
    )
    .expect_err("a plan cannot cross generations");
    let super::AspClientOperationError::Message(message) = error else {
        panic!("expected typed message")
    };
    assert!(message.contains("authority mismatch"), "{message}");
}

#[test]
fn resident_query_rejects_a_nonruntime_capability_row_even_with_a_valid_digest() {
    let mut plan = agent_semantic_tree_sitter::compile_resident_syntax_plan(
        "(function_item) @item",
        GENERATION_DIGEST,
        &rust_capability(),
    )
    .expect("compile resident query plan");
    plan.required_capability_rows
        .push("rust.fact.query-keys".to_owned());
    plan.required_capability_rows.sort();
    plan.plan_digest = plan.recompute_plan_digest().expect("recompute test digest");

    let error = validate_resident_query_plan(&plan, GENERATION_DIGEST, &provider())
        .expect_err("provider-local facts cannot execute in Runtime");
    let super::AspClientOperationError::Message(message) = error else {
        panic!("expected typed message")
    };
    assert!(message.contains("non-runtime capability row"), "{message}");
}
