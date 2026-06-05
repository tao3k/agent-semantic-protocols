use crate::{
    ASP_SYNTAX_QUERY_CAPTURES_ARG, ASP_SYNTAX_QUERY_FIELDS_ARG, ASP_SYNTAX_QUERY_NODE_TYPES_ARG,
    ASP_SYNTAX_QUERY_PREDICATES_JSON_ARG, ClientMethod, append_syntax_query_plan_args,
};

#[test]
fn cache_flush_method_uses_stable_wire_spelling() {
    let value = serde_json::to_value(ClientMethod::CacheFlush).expect("serialize method");

    assert_eq!(value, serde_json::json!("cache-flush"));
}

#[test]
fn appends_asp_syntax_query_plan_for_inline_query() {
    let args = append_syntax_query_plan_args(
        &ClientMethod::Query,
        vec![
            "--treesitter-query".to_string(),
            "(function_item name: (identifier) @function.name)".to_string(),
            ".".to_string(),
        ],
    )
    .expect("valid query");

    assert_eq!(
        args,
        vec![
            "--treesitter-query",
            "(function_item name: (identifier) @function.name)",
            ".",
            ASP_SYNTAX_QUERY_CAPTURES_ARG,
            "function.name",
            ASP_SYNTAX_QUERY_NODE_TYPES_ARG,
            "function_item,identifier",
            ASP_SYNTAX_QUERY_FIELDS_ARG,
            "name",
        ]
    );
}

#[test]
fn appends_predicate_abi_for_inline_query() {
    let args = append_syntax_query_plan_args(
        &ClientMethod::Query,
        vec![
            "--treesitter-query".to_string(),
            r#"(function_item name: (identifier) @function.name (#eq? @function.name "parse_query"))"#
                .to_string(),
            ".".to_string(),
        ],
    )
    .expect("valid query");

    assert!(args.windows(2).any(|window| window
        == [
            ASP_SYNTAX_QUERY_PREDICATES_JSON_ARG,
            r#"[{"capture":"function.name","op":"eq","values":[{"kind":"string","value":"parse_query"}]}]"#
        ]));
}

#[test]
fn appends_capture_operand_predicate_abi_for_inline_query() {
    let args = append_syntax_query_plan_args(
        &ClientMethod::Query,
        vec![
            "--treesitter-query".to_string(),
            r#"(function_item
                name: (identifier) @function.name
                (#not-eq? @function.name @other.name))"#
                .to_string(),
            ".".to_string(),
        ],
    )
    .expect("valid query");

    assert!(args.windows(2).any(|window| window
        == [
            ASP_SYNTAX_QUERY_PREDICATES_JSON_ARG,
            r#"[{"capture":"function.name","op":"not-eq","values":[{"kind":"capture","value":"other.name"}]}]"#
        ]));
}

#[test]
fn appends_any_predicate_abi_for_inline_query() {
    let args = append_syntax_query_plan_args(
        &ClientMethod::Query,
        vec![
            "--treesitter-query".to_string(),
            r#"(function_item
                name: (identifier) @function.name
                (#any-eq? @function.name "parse_query")
                (#any-match? @function.name "^parse_"))"#
                .to_string(),
            ".".to_string(),
        ],
    )
    .expect("valid query");

    assert!(args.windows(2).any(|window| window
        == [
            ASP_SYNTAX_QUERY_PREDICATES_JSON_ARG,
            r#"[{"capture":"function.name","op":"any-eq","values":[{"kind":"string","value":"parse_query"}]},{"capture":"function.name","op":"any-match","values":[{"kind":"string","value":"^parse_"}]}]"#
        ]));
}

#[test]
fn leaves_non_query_requests_unchanged() {
    let args = vec![
        "owner".to_string(),
        "src/lib.rs".to_string(),
        ".".to_string(),
    ];

    assert_eq!(
        append_syntax_query_plan_args(&ClientMethod::Search, args.clone()).expect("search args"),
        args
    );
}
