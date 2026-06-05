use crate::{
    ASP_SYNTAX_QUERY_CAPTURES_ARG, ASP_SYNTAX_QUERY_FIELDS_ARG, ASP_SYNTAX_QUERY_NODE_TYPES_ARG,
    ClientMethod, append_syntax_query_plan_args,
};

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
