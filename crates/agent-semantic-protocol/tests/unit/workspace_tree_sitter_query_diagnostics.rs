use std::fs;
use std::process::Command;

const IMPOSSIBLE_RUST_IDENTIFIER_QUERY: &str = r#"
(function_item
  name: (identifier) @declaration.name
  (#eq? @declaration.name "direct-source-read"))
"#;

const AGENT_SESSION_LOOKUP_QUERY: &str = r#"
((type_identifier) @reference.name
  (#eq? @reference.name "AgentSessionLookupRequest"))
"#;

#[test]
fn zero_match_tree_sitter_query_explains_structural_semantics() {
    let workspace = std::env::temp_dir().join(format!(
        "asp-tree-sitter-query-diagnostics-{}",
        std::process::id()
    ));
    fs::create_dir_all(&workspace).expect("create query workspace");
    fs::write(workspace.join("lib.rs"), "fn direct_source_read() {}\n")
        .expect("write Rust fixture");

    let mut command = Command::new(env!("CARGO_BIN_EXE_asp"));
    command.env_clear();
    for variable in ["HOME", "PATH", "TMPDIR", "CARGO_HOME", "RUSTUP_HOME"] {
        if let Some(value) = std::env::var_os(variable) {
            command.env(variable, value);
        }
    }
    let output = command
        .current_dir(&workspace)
        .args([
            "search",
            "--language",
            "rust",
            "--treesitter-query",
            IMPOSSIBLE_RUST_IDENTIFIER_QUERY,
            "--workspace",
        ])
        .arg(&workspace)
        .output()
        .expect("run structural query");
    fs::remove_dir_all(&workspace).expect("remove query workspace");

    assert!(
        output.status.success(),
        "stdout={}\nstderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
    let stdout = String::from_utf8(output.stdout).expect("utf-8 stdout");
    assert_eq!(stdout.matches("[search-treesitter]").count(), 1);
    assert!(stdout.contains("status=no-matches"), "stdout={stdout}");
    assert!(
        stdout.contains("mode: structural Tree-sitter reasoning search"),
        "stdout={stdout}",
    );
    assert!(
        stdout.contains("use `_` rather than `-`"),
        "stdout={stdout}",
    );
    assert!(stdout.contains("asp rust search pipe"), "stdout={stdout}");
}

#[test]
fn language_tree_sitter_search_uses_explicit_rust_facade() {
    let workspace = std::env::temp_dir().join(format!(
        "asp-tree-sitter-search-language-inference-{}",
        std::process::id()
    ));
    fs::create_dir_all(&workspace).expect("create search workspace");
    fs::write(
        workspace.join("lib.rs"),
        "pub struct AgentSessionLookupRequest;\n",
    )
    .expect("write Rust fixture");
    crate::provider_command::support::write_activation(
        &workspace,
        &[
            crate::provider_command::support::provider("rust", Vec::new()),
            crate::provider_command::support::provider("typescript", Vec::new()),
        ],
    );

    let mut command = Command::new(env!("CARGO_BIN_EXE_asp"));
    command.env_clear();
    for variable in ["HOME", "PATH", "TMPDIR", "CARGO_HOME", "RUSTUP_HOME"] {
        if let Some(value) = std::env::var_os(variable) {
            command.env(variable, value);
        }
    }
    command.env("PRJ_CACHE_HOME", workspace.join(".cache"));
    let output = command
        .current_dir(&workspace)
        .args([
            "rust",
            "search",
            "--treesitter-query",
            AGENT_SESSION_LOOKUP_QUERY,
            "--workspace",
        ])
        .arg(&workspace)
        .output()
        .expect("run root structural search");
    fs::remove_dir_all(&workspace).expect("remove search workspace");

    assert!(
        output.status.success(),
        "stdout={}\nstderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
    let stdout = String::from_utf8(output.stdout).expect("utf-8 stdout");
    assert_eq!(stdout.matches("[search-treesitter]").count(), 1);
    assert!(stdout.contains("status=matches"), "stdout={stdout}");
    assert!(stdout.contains("language=rust"), "stdout={stdout}");
    assert!(stdout.contains("reference.name"), "stdout={stdout}");
}
