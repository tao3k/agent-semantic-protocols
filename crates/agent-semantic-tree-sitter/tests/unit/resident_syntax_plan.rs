// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use agent_semantic_client_protocol::{
    EnhancedQueryCapabilityTable, ResidentSyntaxQueryCondition, ResidentSyntaxQueryResultField,
    ResidentSyntaxQueryScalarValue,
};

use super::compile_resident_syntax_plan;

const GENERATION_DIGEST: &str =
    "blake3-256:1111111111111111111111111111111111111111111111111111111111111111";

fn rust_capability() -> EnhancedQueryCapabilityTable {
    serde_json::from_str(include_str!(
        "../../../../languages/asp-rust/tree-sitter/tree-sitter-rust/enhanced-query-capabilities.v1.json"
    ))
    .expect("Rust enhanced Query capability table")
}

#[test]
fn node_and_scalar_fact_lower_to_a_digest_bound_resident_plan() {
    let plan = compile_resident_syntax_plan(
        r#"(function_item) @item
            (#asp-eq? @item "name" "run")
            (#asp-select! @item "kind" "name" "selector")"#,
        GENERATION_DIGEST,
        &rust_capability(),
    )
    .expect("resident plan");

    assert_eq!(plan.generation_digest, GENERATION_DIGEST);
    assert_eq!(
        plan.selected_fields,
        [
            ResidentSyntaxQueryResultField::Kind,
            ResidentSyntaxQueryResultField::Name,
            ResidentSyntaxQueryResultField::Selector,
        ]
    );
    let ResidentSyntaxQueryCondition::Scalar { value, .. } = &plan.patterns[0].structure else {
        panic!("function node lowers to a scalar kind condition")
    };
    assert_eq!(
        value,
        &ResidentSyntaxQueryScalarValue::Literal("function".to_owned())
    );
    assert!(plan.validate().is_ok());
}

#[test]
fn regex_is_compiled_once_before_resident_execution() {
    let plan = compile_resident_syntax_plan(
        r#"(_) @item (#asp-match? @item "name" "^run(_once)?$")"#,
        GENERATION_DIGEST,
        &rust_capability(),
    )
    .expect("regex resident plan");

    assert_eq!(plan.regex_programs.len(), 1);
    assert_eq!(
        plan.regex_programs[0].engine_id,
        "regex-automata-sparse-dfa"
    );
    assert!(plan.validate().is_ok());
}

#[test]
fn provider_local_result_field_is_rejected_during_plan_admission() {
    let error = compile_resident_syntax_plan(
        r#"(_) @item (#asp-select! @item "queryKeys")"#,
        GENERATION_DIGEST,
        &rust_capability(),
    )
    .expect_err("provider-local facts cannot enter a resident plan");

    assert!(error.contains("capability-not-runtime"), "{error}");
}

#[test]
fn unmaterialized_field_traversal_fails_closed() {
    let error = compile_resident_syntax_plan(
        r#"(function_item name: (identifier) @name) @item"#,
        GENERATION_DIGEST,
        &rust_capability(),
    )
    .expect_err("nested provider-local tree traversal cannot run request-time");

    assert!(error.contains("not-resident"), "{error}");
}
