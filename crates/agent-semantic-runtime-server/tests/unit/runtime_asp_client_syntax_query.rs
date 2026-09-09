// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use super::{
    producer_matches_optional_calibration, resident_pattern_capture, validate_resident_query_plan,
};

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

fn selector(keys: &[&str]) -> agent_semantic_search::NativeSyntaxSelector {
    agent_semantic_search::NativeSyntaxSelector {
        selector: "rust://src/lib.rs#item/function/RuntimeQueryGeneration".to_owned(),
        byte_start: 1,
        byte_end: 2,
        query_keys: keys.iter().map(|key| (*key).to_owned()).collect(),
        derived_projection_digest: format!("blake3-256:{}", "a".repeat(64)),
    }
}

#[test]
fn resident_identifier_regex_uses_published_query_keys_without_a_parser() {
    let plan = agent_semantic_client_core::compile_query_abi_source(
        "((identifier) @symbol (#match? @symbol \"QueryPlaybook|RuntimeQueryGeneration\"))",
    )
    .expect("compile resident query plan");
    assert!(validate_resident_query_plan(&plan).is_ok());
    assert_eq!(
        resident_pattern_capture(
            plan.patterns.first().expect("one pattern"),
            &plan,
            &selector(&["runtime", "query", "generation", "RuntimeQueryGeneration",]),
        )
        .unwrap_or_else(|_| panic!("evaluate resident predicate")),
        Some("symbol".to_owned())
    );
}

#[test]
fn resident_query_rejects_field_traversal_instead_of_calling_a_provider() {
    let plan = agent_semantic_client_core::compile_query_abi_source(
        "((function_item name: (identifier) @name))",
    )
    .expect("compile provider-native query plan");
    let error = validate_resident_query_plan(&plan)
        .expect_err("unsupported request-time semantics must fail typed");
    let super::AspClientOperationError::Message(message) = error else {
        panic!("expected typed message")
    };
    assert!(message.contains("without field traversal"), "{message}");
}
