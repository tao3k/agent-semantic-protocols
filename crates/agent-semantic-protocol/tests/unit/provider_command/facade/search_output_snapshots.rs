use crate::provider_command::support::{
    asp_command, assert_compact_search_action_contract, provider, temp_project_root,
    write_activation, write_rust_owner_frontier_provider,
};

#[test]
fn asp_rg_query_wrapper_stdout_snapshot() {
    let root = temp_project_root("asp-rg-query-wrapper-stdout-snapshot");
    std::fs::create_dir_all(root.join("src")).expect("create src");
    std::fs::write(
        root.join("package.json"),
        r#"{"name":"query-wrapper-fixture"}"#,
    )
    .expect("write package json");
    std::fs::write(
        root.join("src/effect.ts"),
        "export const Fiber = {};\nexport const Queue = {};\nconst staleCache = 'refresh turso cache';\n",
    )
    .expect("write source");

    let output = asp_command(&root)
        .args([
            "rg",
            "-query",
            "Fiber|Queue",
            "-query",
            "stale|refresh|turso|cache",
            "--workspace",
            "src",
        ])
        .output()
        .expect("run asp rg query wrapper snapshot");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("stdout");
    assert_single_line(&stdout, "[search-rg]");
    assert_single_line(&stdout, "nextCommand=");
    assert_single_line(&stdout, "avoid=");
    assert_compact_search_action_contract(&stdout);
    insta::assert_snapshot!(
        "asp_rg_query_wrapper_stdout",
        normalize_search_output(&stdout)
    );

    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn asp_rust_query_file_selector_code_without_item_is_rejected() {
    let root = temp_project_root("asp-rust-query-file-selector-code-rejected");
    write_rust_owner_frontier_provider(&root);
    write_activation(&root, &[provider("rust", Vec::new())]);
    std::fs::create_dir_all(root.join("src")).expect("create src");
    let source = "pub struct QueryExpr;\n\npub fn parse_query_expr() {}\n";
    std::fs::write(root.join("src/core.rs"), source).expect("write source");

    let output = asp_command(&root)
        .env("PRJ_CACHE_HOME", root.join(".cache"))
        .args([
            "rust",
            "query",
            "--selector",
            "src/core.rs",
            "--workspace",
            ".",
            "--code",
        ])
        .output()
        .expect("run asp rust owner selector query snapshot");

    assert!(
        !output.status.success(),
        "stdout: {}",
        String::from_utf8_lossy(&output.stdout)
    );
    let stderr = String::from_utf8(output.stderr).expect("stderr");
    assert!(
        stderr.contains("invalid query --code selector `src/core.rs`"),
        "{stderr}"
    );

    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn asp_rust_query_item_selector_code_returns_item_source() {
    let root = temp_project_root("asp-rust-query-item-selector-code-source");
    write_rust_owner_frontier_provider(&root);
    write_activation(&root, &[provider("rust", Vec::new())]);
    std::fs::create_dir_all(root.join("src")).expect("create src");
    std::fs::write(
        root.join("src/core.rs"),
        "pub struct QueryExpr;\n\npub fn parse_query_expr() {}\n",
    )
    .expect("write source");

    let output = asp_command(&root)
        .env("PRJ_CACHE_HOME", root.join(".cache"))
        .args([
            "rust",
            "query",
            "--selector",
            "rust://src/core.rs#item/function/parse_query_expr",
            "--workspace",
            ".",
            "--code",
        ])
        .output()
        .expect("run asp rust exact item selector query");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("stdout");
    assert_eq!(stdout, "pub fn parse_query_expr() {}\n");
    assert!(!stdout.contains("[search-owner]"), "{stdout}");

    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn asp_rust_query_direct_source_read_returns_bounded_source_window() {
    let root = temp_project_root("asp-rust-query-direct-read-source-window");
    write_rust_owner_frontier_provider(&root);
    write_activation(&root, &[provider("rust", Vec::new())]);
    std::fs::create_dir_all(root.join("src")).expect("create src");
    std::fs::write(
        root.join("src/core.rs"),
        "pub struct QueryExpr;\n\npub fn parse_query_expr() {}\n",
    )
    .expect("write source");

    let output = asp_command(&root)
        .env("PRJ_CACHE_HOME", root.join(".cache"))
        .args([
            "rust",
            "query",
            "--from-hook",
            "direct-source-read",
            "--fallback-reason",
            "bounded-window-after-selector",
            "--selector",
            "src/core.rs:1:1",
            "--workspace",
            ".",
            "--code",
        ])
        .output()
        .expect("run asp rust direct source read");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8(output.stdout).expect("stdout"),
        "pub struct QueryExpr;\n"
    );

    let _ = std::fs::remove_dir_all(root);
}

fn assert_single_line(stdout: &str, prefix: &str) {
    assert_eq!(
        stdout
            .lines()
            .filter(|line| line.starts_with(prefix))
            .count(),
        1,
        "expected exactly one `{prefix}` line in:\n{stdout}"
    );
}

fn normalize_search_output(stdout: &str) -> String {
    let mut normalized = stdout
        .lines()
        .map(normalize_search_line)
        .collect::<Vec<_>>()
        .join("\n");
    normalized.push('\n');
    normalized
}

fn normalize_search_line(line: &str) -> String {
    if line.starts_with("sourceTrace=finder:used[") {
        return "sourceTrace=finder:used[collectMs=<ms>;elapsedMs=<ms>;qualityMs=<ms>]".to_string();
    }

    ["collectMs", "elapsedMs", "qualityMs"]
        .into_iter()
        .fold(line.to_string(), normalize_numeric_field)
}

fn normalize_numeric_field(line: String, key: &str) -> String {
    let needle = format!("{key}=");
    let mut normalized = line;
    let mut cursor = 0;
    while let Some(relative) = normalized[cursor..].find(&needle) {
        let value_start = cursor + relative + needle.len();
        let value_end = normalized[value_start..]
            .find(|character: char| !character.is_ascii_digit())
            .map(|relative_end| value_start + relative_end)
            .unwrap_or(normalized.len());
        if value_start == value_end {
            cursor = value_end;
            continue;
        }
        normalized.replace_range(value_start..value_end, "<ms>");
        cursor = value_start + "<ms>".len();
    }
    normalized
}
