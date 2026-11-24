// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

fn owned_args(parts: &[&str]) -> Vec<String> {
    parts.iter().map(|part| (*part).to_owned()).collect()
}

fn assert_selected(parts: &[&str], expected_name: &str, usage: &str) {
    let mut command = help_model::selected_command(&owned_args(parts));
    assert_eq!(command.get_name(), expected_name, "args={parts:?}");

    let help = command.render_help().to_string();
    assert!(help.contains("Usage:"), "help={help}");
    assert!(help.contains("Options:"), "help={help}");
    assert!(help.contains(usage), "help={help}");
    assert!(!help.starts_with("usage:"), "help={help}");
}

#[test]
fn root_and_first_level_paths_select_their_own_commands() {
    assert_selected(&["--help"], "asp", "asp");
    let mut root = help_model::selected_command(&owned_args(&["--help"]));
    let root_help = root.render_long_help().to_string();
    assert!(!root_help.contains("live-corpus"), "help={root_help}");
    for command in [
        "providers",
        "tools",
        "wrap",
        "cache",
        "cloud",
        "config",
        "install",
        "paths",
        "healthcheck",
        "ast-patch",
        "search",
        "query",
        "gerbil-scheme",
        "julia",
        "md",
        "org",
        "python",
        "rust",
        "typescript",
    ] {
        assert_selected(&[command, "--help"], command, &format!("asp {command}"));
    }
    let mut config = help_model::selected_command(&owned_args(&["config", "--help"]));
    let config_help = config.render_long_help().to_string();
    assert!(!config_help.contains("agents"), "help={config_help}");
}

#[test]
fn removed_agent_hook_and_session_roots_fail_closed_instead_of_rendering_root_help() {
    for removed in ["agent", "hook", "session"] {
        let error = help_model::print_help_if_requested(&owned_args(&[removed, "--help"]))
            .expect_err("retired command root must not look available");
        assert_eq!(error, format!("unknown ASP command `{removed}`"));
    }
}

#[test]
fn search_playbook_help_owns_the_composed_root_contract() {
    let mut command = help_model::selected_command(&owned_args(&["search", "playbook", "--help"]));
    let help = command.render_help().to_string();
    for token in [
        "SCHEME_COMPOSITION_EXPRESSION",
        "One (search ...) expression",
        "(producers (language rust))",
        "(topology (owners (kind file)",
        "((function_item) @item (#asp-select!",
        "Choose topology for generation-bound file/path membership",
        "rg for regex truth",
        "There is no mandatory engine pair",
        "Intersect is an explicit conjunction",
        "graph remains the final barrier",
    ] {
        assert!(help.contains(token), "missing {token}: {help}");
    }
    for removed in ["--language", "--rg", "--tantivy", "--syntax", "--graph"] {
        assert!(!help.contains(removed), "legacy flag leaked: {help}");
    }
    for removed in ["native-query", "(where (kind function))"] {
        assert!(!help.contains(removed), "retired Query DSL leaked: {help}");
    }

    let mut language = help_model::selected_command(&owned_args(&["rust", "--help"]));
    let language_help = language.render_help().to_string();
    assert!(
        !language_help.contains("search"),
        "language-first Search leaked: {language_help}"
    );
    assert!(
        !language_help.contains("query"),
        "language-first Query leaked: {language_help}"
    );
    assert!(
        !language_help.contains("check"),
        "removed language Check leaked: {language_help}"
    );
    assert!(
        !language_help.contains("evidence"),
        "removed language Evidence leaked: {language_help}"
    );
}

#[test]
fn query_playbook_help_exposes_the_exact_search_handoff_grammar() {
    let mut command = help_model::selected_command(&owned_args(&["query", "playbook", "--help"]));
    let help = command.render_help().to_string();
    for token in [
        "QUERY_SCHEME_EXPRESSION",
        "One (query ...) expression",
        "(select (selectors",
        "(projection source)",
        "copied unchanged from Search results",
        "selectors, projection, and output",
    ] {
        assert!(help.contains(token), "missing {token}: {help}");
    }
    for removed in ["--selector", "--projection", "--language"] {
        assert!(!help.contains(removed), "legacy flag leaked: {help}");
    }
}

#[test]
fn install_language_path_selects_language_command() {
    assert_selected(
        &["install", "language", "--help"],
        "language",
        "asp install language",
    );
}

#[test]
fn language_leaf_path_selects_leaf_command() {
    for language in ["gerbil-scheme", "julia", "python", "rust", "typescript"] {
        for leaf in [
            "guide",
            "cache",
            "info",
            "bench",
            "projection",
            "agent",
            "ast-patch",
        ] {
            assert_selected(
                &[language, leaf, "--help"],
                leaf,
                &format!("asp {language} {leaf}"),
            );
        }
    }
}

#[test]
fn removed_language_leaves_do_not_select_command_help() {
    for language in ["gerbil-scheme", "julia", "python", "rust", "typescript"] {
        for leaf in ["check", "evidence"] {
            assert_selected(
                &[language, leaf, "--help"],
                language,
                &format!("asp {language}"),
            );
        }
    }
}

#[test]
fn non_help_invocations_are_not_intercepted() {
    for parts in [
        &["install", "binary"][..],
        &[
            "search",
            "playbook",
            "(search (producers (language rust)) (intersect (rg \"owner\" ) (tantivy \"title:owner^2 OR body:authority\")))",
        ][..],
        &["graph", "render", "--packet", "-"][..],
        &["rust", "search", "--", "--help"][..],
    ] {
        assert!(
            !help_model::print_help_if_requested(&owned_args(parts))
                .expect("non-help routing should not fail"),
            "args={parts:?}",
        );
    }
}
use super as help_model;
