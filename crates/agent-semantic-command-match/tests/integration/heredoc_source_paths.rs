use agent_semantic_command_match::command_source_paths;

#[test]
fn parser_owned_heredoc_candidates_expose_quoted_source_paths() {
    let command =
        "python - <<'PY'\nfrom pathlib import Path\nPath('src/pkg/report.py').read_text()\nPY";

    let candidates = command_source_paths(command, &[]);

    assert!(
        candidates.iter().any(|path| path == "src/pkg/report.py"),
        "{candidates:?}"
    );
}
