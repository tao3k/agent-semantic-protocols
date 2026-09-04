use super::project_registry_gc_args::parse_project_registry_clean_args;
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

#[test]
fn clean_day_without_value_retains_one_day() {
    let parsed = parse_project_registry_clean_args(&["--day".to_string()])
        .expect("parse clean arguments")
        .expect("non-help invocation");
    assert_eq!(parsed.day, 1);
}

#[test]
fn clean_day_accepts_a_larger_explicit_retention() {
    let parsed = parse_project_registry_clean_args(&["--day".to_string(), "14".to_string()])
        .expect("parse clean arguments")
        .expect("non-help invocation");
    assert_eq!(parsed.day, 14);
}

#[test]
fn clean_requires_a_positive_day_retention() {
    let missing = parse_project_registry_clean_args(&[]).expect_err("--day should be required");
    assert!(missing.contains("--day [<DAYS>]"), "{missing}");

    let zero = parse_project_registry_clean_args(&["--day=0".to_string()])
        .expect_err("zero-day cleanup should be rejected");
    assert!(zero.contains("invalid value '0'"), "{zero}");
    assert!(zero.contains("--day"), "{zero}");
}
