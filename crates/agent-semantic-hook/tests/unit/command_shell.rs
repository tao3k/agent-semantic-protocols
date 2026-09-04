use crate::command::semantic_shell_tokens;

#[test]
fn bash_ast_tokens_strip_quotes_from_source_dump_range() {
    assert_eq!(
        semantic_shell_tokens("sed -n '1,40p' src/lib.rs"),
        vec!["sed", "-n", "1,40p", "src/lib.rs"]
    );
}

#[test]
fn bash_ast_tokens_surface_outer_and_nested_wrapper_stages() {
    assert_eq!(
        semantic_shell_tokens("bash -lc \"sed -n '1,40p' src/lib.rs\""),
        vec![
            "bash",
            "-lc",
            "sed -n '1,40p' src/lib.rs",
            "sed",
            "-n",
            "1,40p",
            "src/lib.rs"
        ]
    );
}

#[test]
fn bash_ast_tokens_preserve_absolute_shell_and_nested_command_modes() {
    for command in [
        "/bin/bash -c 'cargo test -p agent-semantic-hook'",
        "/bin/bash -lc 'cargo test -p agent-semantic-hook'",
        "/usr/bin/env /bin/zsh -c 'cargo test -p agent-semantic-hook'",
    ] {
        assert_eq!(
            semantic_shell_tokens(command),
            if command.starts_with("/usr/bin/env") {
                vec![
                    "/usr/bin/env",
                    "/bin/zsh",
                    "-c",
                    "cargo test -p agent-semantic-hook",
                    "cargo",
                    "test",
                    "-p",
                    "agent-semantic-hook",
                ]
            } else {
                vec![
                    "/bin/bash",
                    if command.contains(" -lc ") {
                        "-lc"
                    } else {
                        "-c"
                    },
                    "cargo test -p agent-semantic-hook",
                    "cargo",
                    "test",
                    "-p",
                    "agent-semantic-hook",
                ]
            },
            "{command}"
        );
    }
}

#[test]
fn bash_ast_tokens_keep_playbook_stage_and_following_pipeline_words() {
    assert_eq!(
        semantic_shell_tokens("asp search playbook --language rust workspace | rg HookDecision src/lib.rs"),
        vec![
            "asp",
            "rust",
            "search",
            "playbook",
            "workspace",
            "|",
            "rg",
            "HookDecision",
            "src/lib.rs"
        ]
    );
}

#[test]
fn bash_ast_tokens_preserve_standalone_env_pipeline_stage() {
    assert_eq!(
        semantic_shell_tokens("env | rg PATH"),
        vec!["env", "|", "rg", "PATH"]
    );
    assert_eq!(
        semantic_shell_tokens("printenv | rg '^PATH='"),
        vec!["printenv", "|", "rg", "^PATH="]
    );
}

#[test]
fn bash_ast_tokens_surface_nl_sed_python_source_dump_pipeline() {
    assert_eq!(
        semantic_shell_tokens(
            "nl -ba packages/python/tools/src/tools/semantic_sandtable/step_agent_cli.py | sed -n '1,130p'",
        ),
        vec![
            "nl",
            "-ba",
            "packages/python/tools/src/tools/semantic_sandtable/step_agent_cli.py",
            "|",
            "sed",
            "-n",
            "1,130p",
        ]
    );
}

#[test]
fn bash_ast_tokens_surface_nested_command_stages() {
    assert_eq!(
        semantic_shell_tokens("echo $(cat src/lib.rs) && cat <(sed -n '1,3p' tests/unit.rs)"),
        vec![
            "echo",
            ";",
            "cat",
            "src/lib.rs",
            ";",
            "&&",
            "cat",
            ";",
            "sed",
            "-n",
            "1,3p",
            "tests/unit.rs",
            ";",
        ]
    );
    assert_eq!(
        semantic_shell_tokens("(cat src/lib.rs) || true"),
        vec!["cat", "src/lib.rs", "||", "true"]
    );
}

#[test]
fn bash_ast_tokens_keep_quoted_search_playbook_stage() {
    assert_eq!(
        semantic_shell_tokens(
            "asp search playbook --language typescript 'Effect concurrency Fiber' --workspace .",
        ),
        vec![
            "asp",
            "typescript",
            "search",
            "playbook",
            "Effect concurrency Fiber",
            "--workspace",
            ".",
        ]
    );
}

#[test]
fn bash_ast_tokens_decode_escaped_path_space() {
    assert_eq!(
        semantic_shell_tokens("cat src/my\\ file.rs"),
        vec!["cat", "src/my file.rs"]
    );
    assert_eq!(
        semantic_shell_tokens("tail -n +115 src/my\\ file.rs | head -n 126"),
        vec![
            "tail",
            "-n",
            "+115",
            "src/my file.rs",
            "|",
            "head",
            "-n",
            "126"
        ]
    );
}

#[test]
fn bash_ast_tokens_keep_heredoc_interpreter_command() {
    assert!(
        semantic_shell_tokens(
            "python3 - <<'PY'\nfrom pathlib import Path\nprint(Path('src/lib.rs').read_text())\nPY"
        )
        .iter()
        .any(|token| token == "python3")
    );
}
