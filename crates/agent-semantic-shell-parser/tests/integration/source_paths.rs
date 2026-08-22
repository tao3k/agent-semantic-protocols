use agent_semantic_shell_parser::command_source_paths;

#[test]
fn extracts_paths_embedded_in_read_call_arguments() {
    for command in [
        "custom-runner Path('crates/agent-semantic-hook/src/lib.rs').read_text()",
        "custom-runner read_to_string('crates/agent-semantic-hook/src/lib.rs')",
    ] {
        assert_eq!(
            command_source_paths(command, &[]),
            ["crates/agent-semantic-hook/src/lib.rs"],
            "{command}"
        );
    }
}

#[test]
fn excludes_quoted_heredoc_delimiters_from_source_paths() {
    let command =
        "python3 - <<'PY'\nfrom pathlib import Path\nprint(Path('src/app.py').read_text())\nPY";
    assert_eq!(command_source_paths(command, &[]), ["src/app.py"]);
}
