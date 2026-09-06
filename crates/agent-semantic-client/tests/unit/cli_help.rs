// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-only

use super::install_command;

#[test]
fn install_language_rejects_removed_record_installed_receipt_bridge() {
    let error = install_command()
        .try_get_matches_from([
            "install",
            "language",
            "rust",
            "--record-installed-receipt",
            "/tmp/asp-rust",
        ])
        .expect_err("removed receipt bridge must not remain accepted");
    assert_eq!(error.kind(), clap::error::ErrorKind::UnknownArgument);
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
fn cache_clean_is_clap_owned_and_requires_an_explicit_positive_retention() {
    agent_semantic_client::cache_clean_clap_command()
        .try_get_matches_from(["clean", "--day"])
        .expect("clean --day must select the one-day retention window");
    agent_semantic_client::cache_clean_clap_command()
        .try_get_matches_from(["clean", "--day", "14"])
        .expect("clean must accept a larger explicit retention window");

    let missing = agent_semantic_client::cache_clean_clap_command()
        .try_get_matches_from(["clean"])
        .expect_err("clean must require --day");
    assert_eq!(
        missing.kind(),
        clap::error::ErrorKind::MissingRequiredArgument
    );

    let zero = agent_semantic_client::cache_clean_clap_command()
        .try_get_matches_from(["clean", "--day=0"])
        .expect_err("clean must reject a zero-day retention window");
    assert_eq!(zero.kind(), clap::error::ErrorKind::ValueValidation);
}

#[test]
fn cache_clean_is_the_only_state_home_cleanup_surface() {
    super::cache_command()
        .try_get_matches_from(["cache", "clean", "--day"])
        .expect("State Home cleanup must be nested under asp cache");

    let error = super::root_command()
        .try_get_matches_from(["asp", "clean", "--day"])
        .expect_err("top-level asp clean must not remain as a second surface");
    assert_eq!(error.kind(), clap::error::ErrorKind::InvalidSubcommand);
}

#[test]
fn install_language_scope_is_owned_only_by_state_home_runtime() {
    install_command()
        .try_get_matches_from(["install", "language", "rust"])
        .expect("State Home is the only provider artifact scope");
    install_command()
        .try_get_matches_from(["install", "language", "rust", "--global"])
        .expect_err("the legacy global scope flag must be removed");
    install_command()
        .try_get_matches_from(["install", "language", "rust", "--project", "/tmp/project"])
        .expect_err("Runtime workspace admission, not installation, owns project activation");
    install_command()
        .try_get_matches_from(["install", "language", "rust", "/tmp/project"])
        .expect_err("positional project roots must not remain accepted");

    let mut install = install_command();
    let language = install
        .find_subcommand_mut("language")
        .expect("install language subcommand");
    let mut help = Vec::new();
    language
        .write_long_help(&mut help)
        .expect("render install language help");
    let help = String::from_utf8(help).expect("utf-8 install help");
    assert!(
        !help.contains("--global"),
        "help must not expose the removed global scope: {help}"
    );
    assert!(
        !help.contains("--project <PATH>"),
        "help must not expose project-local artifact publication: {help}"
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

#[test]
fn org_and_markdown_are_document_surfaces_not_language_facades() {
    for document in ["org", "md"] {
        let command = super::root_document_facade_command(document);
        let subcommands = command
            .get_subcommands()
            .map(|subcommand| subcommand.get_name())
            .collect::<Vec<_>>();
        assert_eq!(subcommands, ["guide", "search", "query"]);
        for language_only in [
            "check",
            "cache",
            "info",
            "bench",
            "projection",
            "agent",
            "ast-patch",
            "evidence",
        ] {
            assert!(
                !subcommands.contains(&language_only),
                "{document} document surface leaked language-only command {language_only}"
            );
        }
    }
    assert!(!super::is_language_facade("org"));
    assert!(!super::is_language_facade("md"));
    assert!(super::is_document_facade("org"));
    assert!(super::is_document_facade("md"));
}
