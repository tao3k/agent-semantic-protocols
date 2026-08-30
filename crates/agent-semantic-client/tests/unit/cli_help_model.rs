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
        "providers",
        "tools",
        "wrap",
        "cache",
        "cloud",
        "hook",
        "config",
        "session",
        "install",
        "paths",
        "healthcheck",
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
    assert_selected(
        &["config", "agents", "--help"],
        "agents",
        "asp config agents",
    );
    let mut config = help_model::selected_command(&owned_args(&["config", "--help"]));
    let config_help = config.render_long_help().to_string();
    assert!(config_help.contains("agents"));
    assert_selected(
        &["config", "agents", "sync", "--help"],
        "sync",
        "asp config agents sync",
    );
    let mut session = help_model::selected_command(&owned_args(&["session", "--help"]));
    let session_help = session.render_long_help().to_string();
    assert!(session_help.contains("--children"));
    assert!(session_help.contains("--agents"));
}

#[test]
fn removed_agent_root_fails_closed_instead_of_rendering_root_help() {
    let error = help_model::print_help_if_requested(&owned_args(&["agent", "--help"]))
        .expect_err("the removed asp agent root must not look available");
    assert_eq!(error, "unknown ASP command `agent`");
}

#[test]
fn hook_break_glass_help_is_a_public_typed_command() {
    assert_selected(
        &["hook", "break-glass", "--help"],
        "asp hook break-glass",
        "asp hook break-glass",
    );
    let mut command = help_model::selected_command(&owned_args(&["hook", "break-glass", "--help"]));
    let help = command.render_long_help().to_string();
    assert!(help.contains("mint"), "help={help}");
    assert!(
        help.contains("one-shot Hook defect capability"),
        "help={help}"
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
fn codex_plugin_help_is_global_and_never_defaults_to_the_current_directory() {
    let mut command = help_model::selected_command(&owned_args(&["install", "plugin", "--help"]));
    let help = command.render_help().to_string();

    assert!(help.contains("globally"), "help={help}");
    assert!(help.contains("ASP_STATE_HOME [dev].root"), "help={help}");
    assert!(!help.contains("--global-plugin"), "help={help}");
    assert!(!help.contains("--project-plugin"), "help={help}");
    assert!(!help.contains("[default: .]"), "help={help}");
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
