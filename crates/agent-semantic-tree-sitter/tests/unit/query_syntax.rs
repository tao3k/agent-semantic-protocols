use crate::compile_query_abi_source;

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
