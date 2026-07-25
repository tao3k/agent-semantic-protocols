use super::project_registry_gc_args::parse_project_registry_gc_args;

#[test]
fn parses_gc_flags_with_clap() {
    let args = vec![
        "--apply".to_string(),
        "--grace-days".to_string(),
        "14".to_string(),
    ];
    let parsed = parse_project_registry_gc_args(&args)
        .expect("parse arguments")
        .expect("non-help invocation");
    assert!(parsed.apply);
    assert_eq!(parsed.grace_days, 14);
}

#[test]
fn rejects_unknown_gc_flags_with_clap_diagnostic() {
    let error = parse_project_registry_gc_args(&["--unknown".to_string()])
        .expect_err("unknown argument should fail");
    assert!(error.contains("unexpected argument '--unknown'"));
    assert!(error.contains("Usage: asp cache gc"));
}
