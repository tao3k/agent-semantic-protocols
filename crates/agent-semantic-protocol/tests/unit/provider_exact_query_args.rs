#[path = "../../src/command/provider_exact_args.rs"]
mod subject;

fn args(values: &[&str]) -> Vec<String> {
    values.iter().map(|value| (*value).to_owned()).collect()
}

#[test]
fn accepts_only_the_typed_exact_projection_surface() {
    let parsed = subject::parse_exact_query_args(&args(&[
        "query",
        "--selector",
        "rust://src/lib.rs#item/function/run",
        "--projection",
        "source",
    ]))
    .expect("typed exact query");
    assert_eq!(parsed.projection, "source");
}

#[test]
fn generated_usage_exposes_only_the_typed_surface() {
    let mut command = subject::exact_query_command();
    let usage = command.render_usage().to_string();
    assert!(usage.contains("--selector <selector>"));
    assert!(usage.contains("--projection <projection>"));
    assert!(!usage.contains("--code"));
    assert!(!usage.contains("--names-only"));
}

#[test]
fn removed_code_flag_is_an_unknown_argument() {
    let error = subject::parse_exact_query_args(&args(&[
        "query",
        "--selector",
        "rust://src/lib.rs#item/function/run",
        "--projection",
        "source",
        "--code",
    ]))
    .expect_err("removed flag must fail before runtime I/O");
    assert!(error.contains("unexpected argument '--code'"));
}

#[test]
fn removed_names_only_flag_is_an_unknown_argument() {
    let error = subject::parse_exact_query_args(&args(&[
        "query",
        "--selector",
        "rust://src/lib.rs#item/function/run",
        "--projection",
        "source",
        "--names-only",
    ]))
    .expect_err("removed flag must fail before runtime I/O");
    assert!(error.contains("unexpected argument '--names-only'"));
}
