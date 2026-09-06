use super::{ProgressiveQueryRequest, QueryOutputFormat, parse_progressive_query_args};

fn args(values: &[&str]) -> Vec<String> {
    values.iter().map(|value| (*value).to_owned()).collect()
}

#[test]
fn selector_query_needs_no_language_authority() {
    assert_eq!(
        parse_progressive_query_args(&args(&["query", "--selector", "selector-1"])),
        Ok(ProgressiveQueryRequest::Selector {
            selector: "selector-1".to_owned(),
            projection: "source".to_owned(),
            output_format: QueryOutputFormat::Human,
            workspace: None,
        })
    );
}

#[test]
fn query_playbook_marker_selects_the_same_exact_query_contract() {
    let selector =
        "rust://src/registry.rs#item/method/refresh/scope/implementation-owner/type/Registry";
    assert_eq!(
        parse_progressive_query_args(&args(&["query", "playbook", "--selector", selector])),
        Ok(ProgressiveQueryRequest::Selector {
            selector: selector.to_owned(),
            projection: "source".to_owned(),
            output_format: QueryOutputFormat::Human,
            workspace: None,
        })
    );
}

#[test]
fn syntax_query_preserves_provider_native_grammar() {
    let request = parse_progressive_query_args(&args(&[
        "query",
        "--languages",
        "rust|python",
        "--documents",
        "org|md",
        "--syntax",
        "rust",
        "--treesitter-query",
        "((identifier) @symbol)",
    ]))
    .expect("native syntax query");

    let ProgressiveQueryRequest::Syntax { syntax, .. } = request else {
        panic!("expected syntax mode");
    };
    assert_eq!(syntax[0].producer, "rust");
    assert_eq!(
        syntax[0].argv,
        args(&["--treesitter-query", "((identifier) @symbol)"])
    );
}

#[test]
fn selector_and_syntax_modes_cannot_mix() {
    let error = parse_progressive_query_args(&args(&[
        "query",
        "--selector",
        "selector-1",
        "--languages",
        "rust",
        "--syntax",
        "rust",
        "--treesitter-query",
        "((identifier) @symbol)",
    ]))
    .expect_err("two query authorities");
    assert!(error.contains("exactly one mode"));
}

#[test]
fn query_defaults_to_human_source_and_json_is_explicit() {
    let default = parse_progressive_query_args(&args(&[
        "query",
        "--selector",
        "gerbil-scheme://src/build-api/package-spec.ss#item/function/package-spec",
    ]))
    .expect("default exact query");
    assert!(matches!(
        default,
        ProgressiveQueryRequest::Selector {
            projection,
            output_format: QueryOutputFormat::Human,
            ..
        } if projection == "source"
    ));

    let json = parse_progressive_query_args(&args(&[
        "query",
        "--selector",
        "gerbil-scheme://src/build-api/package-spec.ss#item/function/package-spec",
        "--json",
    ]))
    .expect("explicit machine query");
    assert!(matches!(
        json,
        ProgressiveQueryRequest::Selector {
            output_format: QueryOutputFormat::Json,
            ..
        }
    ));
}
