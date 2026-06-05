use crate::{
    ASP_SYNTAX_QUERY_CAPTURES_ARG, ASP_SYNTAX_QUERY_FIELDS_ARG, ASP_SYNTAX_QUERY_NODE_TYPES_ARG,
    ASP_SYNTAX_QUERY_PREDICATES_JSON_ARG, ClientMethod, LanguageId, append_syntax_query_plan_args,
};

#[test]
fn cache_flush_method_uses_stable_wire_spelling() {
    let value = serde_json::to_value(ClientMethod::CacheFlush).expect("serialize method");

    assert_eq!(value, serde_json::json!("cache-flush"));
}

#[test]
fn appends_asp_syntax_query_plan_for_inline_query() {
    let language_id = LanguageId::from("rust");
    let args = append_syntax_query_plan_args(
        &ClientMethod::Query,
        Some(&language_id),
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
    let language_id = LanguageId::from("rust");
    let args = append_syntax_query_plan_args(
        &ClientMethod::Query,
        Some(&language_id),
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
    let language_id = LanguageId::from("rust");
    let args = append_syntax_query_plan_args(
        &ClientMethod::Query,
        Some(&language_id),
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
    let language_id = LanguageId::from("rust");
    let args = append_syntax_query_plan_args(
        &ClientMethod::Query,
        Some(&language_id),
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
fn appends_asp_syntax_query_plan_for_builtin_rust_catalog() {
    let language_id = LanguageId::from("rust");
    let args = append_syntax_query_plan_args(
        &ClientMethod::Query,
        Some(&language_id),
        vec![
            "--catalog".to_string(),
            "declarations".to_string(),
            ".".to_string(),
        ],
    )
    .expect("valid catalog");

    assert!(args.windows(2).any(|window| window
        == [
            ASP_SYNTAX_QUERY_CAPTURES_ARG,
            "constant.definition,constant.name,constant.type,function.definition,function.modifier,function.name,function.return_type,function.type_parameters,impl.definition,impl.target,impl.trait,impl.type_parameters,item.attribute,item.visibility,module.definition,module.name,trait.bounds,trait.definition,trait.name,trait.type_parameters,type.aliased_type,type.definition,type.name,type.type_parameters"
        ]));
    assert_internal_arg_contains(&args, ASP_SYNTAX_QUERY_NODE_TYPES_ARG, "function_item");
    assert_internal_arg_contains(&args, ASP_SYNTAX_QUERY_NODE_TYPES_ARG, "trait_item");
    assert_internal_arg_contains(&args, ASP_SYNTAX_QUERY_FIELDS_ARG, "name");
}

#[test]
fn appends_asp_syntax_query_plan_for_builtin_typescript_catalog() {
    let language_id = LanguageId::from("typescript");
    let args = append_syntax_query_plan_args(
        &ClientMethod::Query,
        Some(&language_id),
        vec![
            "--catalog".to_string(),
            "declarations".to_string(),
            ".".to_string(),
        ],
    )
    .expect("valid catalog");

    assert_internal_arg_contains(&args, ASP_SYNTAX_QUERY_CAPTURES_ARG, "class.name");
    assert_internal_arg_contains(&args, ASP_SYNTAX_QUERY_CAPTURES_ARG, "function.name");
    assert_internal_arg_contains(&args, ASP_SYNTAX_QUERY_NODE_TYPES_ARG, "class_declaration");
}

#[test]
fn appends_asp_syntax_query_plan_for_builtin_python_catalog() {
    let language_id = LanguageId::from("python");
    let args = append_syntax_query_plan_args(
        &ClientMethod::Query,
        Some(&language_id),
        vec![
            "--catalog".to_string(),
            "imports".to_string(),
            ".".to_string(),
        ],
    )
    .expect("valid catalog");

    assert_internal_arg_contains(&args, ASP_SYNTAX_QUERY_CAPTURES_ARG, "import.name");
    assert_internal_arg_contains(&args, ASP_SYNTAX_QUERY_CAPTURES_ARG, "import.path");
    assert_internal_arg_contains(&args, ASP_SYNTAX_QUERY_NODE_TYPES_ARG, "import_statement");
}

#[test]
fn leaves_non_query_requests_unchanged() {
    let args = vec![
        "owner".to_string(),
        "src/lib.rs".to_string(),
        ".".to_string(),
    ];

    assert_eq!(
        append_syntax_query_plan_args(&ClientMethod::Search, None, args.clone())
            .expect("search args"),
        args
    );
}

fn assert_internal_arg_contains(args: &[String], key: &str, expected_value: &str) {
    let value = args
        .windows(2)
        .find(|window| window[0] == key)
        .map(|window| window[1].as_str())
        .unwrap_or_else(|| panic!("missing internal arg {key}"));
    assert!(
        value.split(',').any(|value| value == expected_value),
        "{key} did not contain {expected_value}: {value}"
    );
}
