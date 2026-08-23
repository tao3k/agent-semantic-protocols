#[path = "../../src/command/provider_exact_args.rs"]
mod subject;

fn args(values: &[&str]) -> Vec<String> {
    values.iter().map(|value| (*value).to_owned()).collect()
}

#[test]
fn accepts_only_the_source_query_surface() {
    let parsed = subject::parse_exact_query_args(&args(&[
        "query",
        "--selector",
        "rust://src/lib.rs#item/function/run",
        "--projection",
        "source",
    ]))
    .expect("typed exact query");
    assert_eq!(
        parsed.structural_selector,
        "rust://src/lib.rs#item/function/run"
    );
}

#[test]
fn generated_usage_exposes_only_the_source_surface() {
    let mut command = subject::exact_query_command();
    let usage = command.render_usage().to_string();
    assert!(usage.contains("--selector <selector>"));
    assert!(usage.contains("--projection <source|callable-skeleton>"));
    assert!(!usage.contains("--json"));
    assert!(!usage.contains("--code"));
    assert!(!usage.contains("--names-only"));
}

#[test]
fn removed_code_flag_is_an_unknown_argument() {
    let error = subject::parse_exact_query_args(&args(&[
        "query",
        "--selector",
        "rust://src/lib.rs#item/function/run",
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
        "--names-only",
    ]))
    .expect_err("removed flag must fail before runtime I/O");
    assert!(error.contains("unexpected argument '--names-only'"));
}

#[test]
fn missing_projection_is_rejected_by_cli_admission() {
    let error = subject::parse_exact_query_args(&args(&[
        "query",
        "--selector",
        "rust://src/lib.rs#item/function/run",
    ]))
    .expect_err("missing projection must fail before runtime I/O");
    assert!(error.contains("required arguments were not provided"));
    assert!(error.contains("--projection <source|callable-skeleton>"));
}

#[test]
fn removed_json_flag_is_an_unknown_argument() {
    let error = subject::parse_exact_query_args(&args(&[
        "query",
        "--selector",
        "rust://src/lib.rs#item/function/run",
        "--json",
    ]))
    .expect_err("removed JSON flag must fail before runtime I/O");
    assert!(error.contains("unexpected argument '--json'"));
}
