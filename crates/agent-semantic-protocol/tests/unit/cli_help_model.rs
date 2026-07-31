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
    for command in [
        "guide",
        "providers",
        "tools",
        "wrap",
        "cache",
        "cloud",
        "hook",
        "agent",
        "install",
        "paths",
        "healthcheck",
        "source-access",
        "ast-patch",
        "graph",
        "fd",
        "rg",
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
    assert_selected(&["agent", "config", "--help"], "config", "asp agent config");
    assert_selected(
        &["agent", "config", "sync", "--help"],
        "sync",
        "asp agent config sync",
    );
}

#[test]
fn install_plugin_path_selects_plugin_command() {
    assert_selected(
        &["install", "plugin", "--help"],
        "plugin",
        "asp install plugin",
    );
}

#[test]
fn codex_plugin_help_states_global_default_and_explicit_project_resolution() {
    let mut command = help_model::selected_command(&owned_args(&["install", "plugin", "--help"]));
    let help = command.render_help().to_string();

    assert!(
        help.contains("Install globally (default when no scope flag is given)"),
        "help={help}",
    );
    assert!(help.contains("--global"), "help={help}");
    assert!(help.contains("--global-plugin"), "help={help}");
    assert!(help.contains("--project"), "help={help}");
    assert!(help.contains("--project-plugin"), "help={help}");
    assert!(help.contains("[default: .]"), "help={help}");
}

#[test]
fn search_help_owns_tree_sitter_discovery() {
    for args in [&["search", "--help"][..], &["rust", "search", "--help"][..]] {
        let mut command = help_model::selected_command(&owned_args(args));
        let help = command.render_help().to_string();

        assert!(help.contains("--treesitter-query <QUERY>"), "help={help}");
        assert!(
            help.contains("workspace-wide structural reasoning search"),
            "help={help}",
        );
        assert!(
            help.contains("query with an exact --selector"),
            "help={help}",
        );
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
fn graph_render_path_selects_render_command() {
    assert_selected(&["graph", "render", "--help"], "render", "asp graph render");
}

#[test]
fn language_leaf_path_selects_leaf_command() {
    for language in ["gerbil-scheme", "julia", "python", "rust", "typescript"] {
        for leaf in [
            "guide",
            "search",
            "query",
            "check",
            "cache",
            "info",
            "bench",
            "projection",
            "agent",
            "ast-patch",
            "evidence",
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
fn non_help_invocations_are_not_intercepted() {
    for parts in [
        &["install", "plugin", "--codex"][..],
        &["rust", "search", "owner"][..],
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
