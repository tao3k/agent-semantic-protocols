use super::turso_migration_args::{CacheMigrationTarget, parse_cache_migration_target};

#[test]
fn parses_turso_0_7_target_with_clap() {
    let parsed = parse_cache_migration_target(&["turso-0-7".to_string()])
        .expect("parse migration target")
        .expect("non-help invocation");
    assert_eq!(parsed, CacheMigrationTarget::Turso07);
}

#[test]
fn rejects_unknown_migration_target_with_clap_diagnostic() {
    let error = parse_cache_migration_target(&["sqlite".to_string()])
        .expect_err("unknown migration target should fail");
    assert!(error.contains("unrecognized subcommand 'sqlite'"));
    assert!(error.contains("Usage: asp cache migrate <COMMAND>"));
}
