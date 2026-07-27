use super::install_command;

#[test]
fn install_language_record_installed_receipt_is_clap_owned_but_hidden() {
    install_command()
        .try_get_matches_from([
            "install",
            "language",
            "rust",
            "--record-installed-receipt",
            "/tmp/rs-harness",
        ])
        .expect("clap must own the root-Justfile receipt bridge");

    let mut help = Vec::new();
    install_command()
        .write_long_help(&mut help)
        .expect("render install help");
    let help = String::from_utf8(help).expect("utf-8 install help");
    assert!(
        !help.contains("--record-installed-receipt"),
        "root-Justfile receipt bridge must stay off the downstream install surface: {help}"
    );
}

#[test]
fn cache_gc_is_clap_owned_and_accepts_no_project_path() {
    super::cache_command()
        .try_get_matches_from(["cache", "gc", "--apply", "--grace-days", "0"])
        .expect("clap must own the singleton State Home GC surface");

    let error = super::cache_command()
        .try_get_matches_from(["cache", "gc", "/tmp/not-a-project-root"])
        .expect_err("cache GC must not accept a project or State Home path");
    assert_eq!(error.kind(), clap::error::ErrorKind::UnknownArgument);
}

#[test]
fn install_language_receipt_reconciliation_is_clap_owned_and_exclusive() {
    install_command()
        .try_get_matches_from(["install", "language", "rust", "--reconcile-receipt"])
        .expect("clap must own provider receipt reconciliation");
    install_command()
        .try_get_matches_from([
            "install",
            "language",
            "rust",
            "--reconcile-receipt",
            "--from-workspace",
        ])
        .expect_err("provider reconciliation and workspace build must be exclusive");
}

#[test]
fn install_language_scope_is_global_by_default_or_explicitly_project_local() {
    install_command()
        .try_get_matches_from(["install", "language", "rust"])
        .expect("global must be the default install scope");
    install_command()
        .try_get_matches_from(["install", "language", "rust", "--global"])
        .expect("--global must explicitly select the default install scope");
    install_command()
        .try_get_matches_from(["install", "language", "rust", "--project", "/tmp/project"])
        .expect("--project PATH must select project-local install scope");
    install_command()
        .try_get_matches_from([
            "install",
            "language",
            "rust",
            "--global",
            "--project",
            "/tmp/project",
        ])
        .expect_err("--global and --project must be mutually exclusive");
    install_command()
        .try_get_matches_from(["install", "language", "rust", "/tmp/project"])
        .expect_err("positional project roots must not remain accepted");

    let mut help = Vec::new();
    install_command()
        .write_long_help(&mut help)
        .expect("render install help");
    let help = String::from_utf8(help).expect("utf-8 install help");
    assert!(
        help.contains("--global"),
        "help must expose --global: {help}"
    );
    assert!(
        help.contains("--project <PATH>"),
        "help must expose explicit project scope: {help}"
    );
    assert!(
        !help.contains("[PROJECT_ROOT]"),
        "legacy positional project root must be absent: {help}"
    );
    assert!(
        !help.contains("--workspace"),
        "legacy --workspace scope must be absent: {help}"
    );
}
