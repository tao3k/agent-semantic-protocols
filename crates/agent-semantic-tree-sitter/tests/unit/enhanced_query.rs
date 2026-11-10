// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use crate::{
    EnhancedQueryExpression, EnhancedQueryOperand, EnhancedQueryPredicateKind,
    parse_enhanced_query_source,
};

#[test]
fn query_language_parser_preserves_patterns_hierarchy_and_predicates() {
    let document = parse_enhanced_query_source(
        r#"
        (function_item) @item
        (#asp-eq? @item "name" "run")
        [(struct_item) @item (enum_item) @item]
        "#,
    )
    .expect("enhanced Query source");
    assert_eq!(document.patterns.len(), 2);
    let EnhancedQueryExpression::NamedNode {
        name,
        captures,
        children,
        ..
    } = &document.patterns[0].structure
    else {
        panic!("first pattern must remain a named node");
    };
    assert_eq!(name, "function_item");
    assert_eq!(captures, &["item"]);
    assert!(children.is_empty());
    let predicate = &document.patterns[0].predicates[0];
    assert_eq!(predicate.name, "asp-eq");
    assert_eq!(predicate.kind, EnhancedQueryPredicateKind::Predicate);
    assert_eq!(
        predicate.operands,
        [
            EnhancedQueryOperand::Capture("item".to_owned()),
            EnhancedQueryOperand::String("name".to_owned()),
            EnhancedQueryOperand::String("run".to_owned()),
        ]
    );
    let EnhancedQueryExpression::Alternation { alternatives, .. } = &document.patterns[1].structure
    else {
        panic!("second pattern must remain an alternation");
    };
    assert_eq!(alternatives.len(), 2);
}

#[test]
fn query_language_parser_rejects_malformed_source() {
    let error =
        parse_enhanced_query_source("(function_item").expect_err("unclosed query must fail");
    assert!(error.message.contains("syntactically invalid"));
}
