use crate::command::{looks_like_command_transcript, semantic_shell_tokens};

#[test]
fn bash_ast_tokens_strip_quotes_from_source_dump_range() {
    assert_eq!(
        semantic_shell_tokens("sed -n '1,40p' src/lib.rs"),
        vec!["sed", "-n", "1,40p", "src/lib.rs"]
    );
}

#[test]
fn bash_ast_tokens_unwrap_login_shell_script() {
    assert_eq!(
        semantic_shell_tokens("bash -lc \"sed -n '1,40p' src/lib.rs\""),
        vec!["sed", "-n", "1,40p", "src/lib.rs"]
    );
}

#[test]
fn bash_ast_tokens_preserve_pipeline_separator() {
    assert_eq!(
        semantic_shell_tokens("asp rust search prime . | rg HookDecision src/lib.rs"),
        vec![
            "asp",
            "rust",
            "search",
            "prime",
            ".",
            "|",
            "rg",
            "HookDecision",
            "src/lib.rs"
        ]
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
fn bash_ast_tokens_keep_quoted_search_pipe_stage() {
    assert_eq!(
        semantic_shell_tokens(
            "asp typescript search pipe 'Effect concurrency Fiber' --workspace . --view seeds",
        ),
        vec![
            "asp",
            "typescript",
            "search",
            "pipe",
            "Effect concurrency Fiber",
            "--workspace",
            ".",
            "--view",
            "seeds",
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

#[test]
fn command_transcript_detector_matches_agent_read_lines() {
    assert!(looks_like_command_transcript(
        "Read src/lib.rs\nSearched for HookDecision"
    ));
    assert!(!looks_like_command_transcript(
        "asp rust query --from-hook direct-source-read --selector src/lib.rs ."
    ));
}
