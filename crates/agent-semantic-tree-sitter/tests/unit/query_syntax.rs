use crate::{SyntaxQueryPredicateOp, SyntaxQueryPredicateValue, compile_query_abi_source};

#[test]
fn compiles_function_name_query_to_abi_plan() {
    let plan = compile_query_abi_source(
        "(function_item name: (identifier) @function.name) @function.definition",
    )
    .expect("query ABI plan");

    assert_eq!(plan.pattern_count(), 1);
    assert_eq!(plan.captures, vec!["function.definition", "function.name"]);
    assert_eq!(plan.node_types, vec!["function_item", "identifier"]);
    assert_eq!(plan.fields, vec!["name"]);
    assert_eq!(plan.patterns[0].index, 0);
    assert_eq!(
        plan.patterns[0].captures,
        vec!["function.definition", "function.name"]
    );
}

#[test]
fn compiles_alternation_predicates_and_comments_without_grammar() {
    let plan = compile_query_abi_source(
        r#"
        ; ASP syntax ABI declaration frontier
        [
          (function_item name: (identifier) @function.name)
          (struct_item name: (type_identifier) @type.name)
        ]
        ((identifier) @local.name
          (#match? @local.name "^run"))
        "#,
    )
    .expect("query ABI plan");

    assert_eq!(plan.pattern_count(), 2);
    assert_eq!(
        plan.captures,
        vec!["function.name", "local.name", "type.name"]
    );
    assert_eq!(
        plan.node_types,
        vec![
            "function_item",
            "identifier",
            "struct_item",
            "type_identifier"
        ]
    );
    assert_eq!(plan.fields, vec!["name"]);
    assert_eq!(plan.patterns[1].captures, vec!["local.name"]);
    assert_eq!(plan.patterns[1].node_types, vec!["identifier"]);
    assert_eq!(plan.predicates.len(), 1);
    assert_eq!(plan.predicates[0].op, SyntaxQueryPredicateOp::Match);
    assert_eq!(plan.predicates[0].capture, "local.name");
    assert_eq!(
        plan.predicates[0].values,
        vec![SyntaxQueryPredicateValue::String("^run".to_string())]
    );
}

#[test]
fn extracts_predicates_for_provider_projection() {
    let plan = compile_query_abi_source(
        r#"(function_item
            name: (identifier) @function.name
            (#eq? @function.name "render_query_local_window")
            (#any-eq? @function.name "render_query_frontier")
            (#any-of? @function.name "parse_query" "run_query")
            (#any-match? @function.name "^render_")
            (#not-eq? @function.name @other.name))"#,
    )
    .expect("query ABI plan");

    assert_eq!(plan.predicates.len(), 5);
    assert_eq!(plan.predicates[0].op, SyntaxQueryPredicateOp::Eq);
    assert_eq!(plan.predicates[0].capture, "function.name");
    assert_eq!(
        plan.predicates[0].values,
        vec![SyntaxQueryPredicateValue::String(
            "render_query_local_window".to_string()
        )]
    );
    assert_eq!(plan.predicates[1].op, SyntaxQueryPredicateOp::AnyEq);
    assert_eq!(plan.predicates[1].capture, "function.name");
    assert_eq!(
        plan.predicates[1].values,
        vec![SyntaxQueryPredicateValue::String(
            "render_query_frontier".to_string()
        )]
    );
    assert_eq!(plan.predicates[2].op, SyntaxQueryPredicateOp::AnyOf);
    assert_eq!(plan.predicates[2].capture, "function.name");
    assert_eq!(
        plan.predicates[2].values,
        vec![
            SyntaxQueryPredicateValue::String("parse_query".to_string()),
            SyntaxQueryPredicateValue::String("run_query".to_string())
        ]
    );
    assert_eq!(plan.predicates[3].op, SyntaxQueryPredicateOp::AnyMatch);
    assert_eq!(plan.predicates[3].capture, "function.name");
    assert_eq!(
        plan.predicates[3].values,
        vec![SyntaxQueryPredicateValue::String("^render_".to_string())]
    );
    assert_eq!(plan.predicates[4].op, SyntaxQueryPredicateOp::NotEq);
    assert_eq!(plan.predicates[4].capture, "function.name");
    assert_eq!(
        plan.predicates[4].values,
        vec![SyntaxQueryPredicateValue::Capture("other.name".to_string())]
    );
}

#[test]
fn rejects_unbalanced_query_source() {
    let error = compile_query_abi_source("(function_item name: (identifier) @function.name")
        .expect_err("unbalanced query should fail");

    assert_eq!(error.message, "unclosed query pattern");
}

#[test]
fn rejects_empty_capture_name() {
    let error = compile_query_abi_source("(identifier) @").expect_err("empty capture should fail");

    assert_eq!(error.message, "empty capture name");
}
