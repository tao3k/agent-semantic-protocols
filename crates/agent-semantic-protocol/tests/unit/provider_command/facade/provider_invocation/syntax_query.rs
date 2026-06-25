use crate::provider_command::support::{
    asp_command, home_local_bin, make_executable, prepend_path, provider, temp_project_root,
    write_activation, write_echo_provider,
};

#[test]
fn language_facade_query_injects_asp_compiled_tree_sitter_plan_for_each_provider() {
    let root = temp_project_root("provider-syntax-query-plan-facade");
    let bin_dir = root.join(".bin");
    write_echo_provider(&bin_dir, "rs-harness", "rust-provider");
    write_echo_provider(&bin_dir, "ts-harness", "typescript-provider");
    write_echo_provider(&bin_dir, "py-harness", "python-provider");
    write_activation(
        &root,
        &[
            provider("rust", Vec::new()),
            provider("typescript", Vec::new()),
            provider("python", Vec::new()),
        ],
    );

    struct SyntaxQueryCase {
        language: &'static str,
        label: &'static str,
        query: &'static str,
        selector: &'static str,
        node_types: &'static str,
        predicate_value: &'static str,
    }

    let cases = [
        SyntaxQueryCase {
            language: "rust",
            label: "rust-provider",
            query: "(function_item name: (identifier) @function.name (#eq? @function.name \"parse_query\"))",
            selector: "src/cli/query.rs",
            node_types: "function_item,identifier",
            predicate_value: "parse_query",
        },
        SyntaxQueryCase {
            language: "typescript",
            label: "typescript-provider",
            query: "(function_declaration name: (identifier) @function.name (#eq? @function.name \"parseTreeSitterQueryArgs\"))",
            selector: "src/cli/protocol-tree-sitter-query.ts",
            node_types: "function_declaration,identifier",
            predicate_value: "parseTreeSitterQueryArgs",
        },
        SyntaxQueryCase {
            language: "python",
            label: "python-provider",
            query: "(function_definition name: (identifier) @function.name (#eq? @function.name \"run_query_command\"))",
            selector: "src/python_lang_project_harness/_cli_query.py",
            node_types: "function_definition,identifier",
            predicate_value: "run_query_command",
        },
    ];

    for case in cases {
        let output = asp_command(&root)
            .env("PRJ_CACHE_HOME", root.join(".cache"))
            .env("PATH", prepend_path(&bin_dir))
            .args([
                case.language,
                "query",
                "--treesitter-query",
                case.query,
                "--selector",
                case.selector,
                ".",
            ])
            .output()
            .unwrap_or_else(|error| panic!("run asp {} syntax query: {error}", case.language));

        assert!(
            output.status.success(),
            "{} stderr: {}",
            case.language,
            String::from_utf8_lossy(&output.stderr)
        );
        let stdout = String::from_utf8(output.stdout).expect("stdout");
        assert!(
            stdout.starts_with(&format!(
                "{} args=[query][--treesitter-query][{}][--selector][{}]",
                case.label, case.query, case.selector
            )),
            "stdout: {stdout}"
        );
        assert!(
            stdout.contains("[--asp-syntax-query-predicates-json]"),
            "stdout: {stdout}"
        );
        assert!(
            stdout.contains("\"capture\":\"function.name\""),
            "stdout: {stdout}"
        );
        assert!(stdout.contains("\"op\":\"eq\""), "stdout: {stdout}");
        assert!(
            stdout.contains(&format!("\"value\":\"{}\"", case.predicate_value)),
            "stdout: {stdout}"
        );
        assert!(
            stdout.contains(&format!(
                "[--asp-syntax-query-captures][function.name][--asp-syntax-query-node-types][{}][--asp-syntax-query-fields][name]",
                case.node_types
            )),
            "stdout: {stdout}"
        );
    }
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn language_facade_query_allows_syntax_code_output_with_exact_selector() {
    let root = temp_project_root("provider-syntax-query-stdout-facade");
    let home_bin = home_local_bin(&root);
    std::fs::create_dir_all(&home_bin).expect("create home local bin");
    let provider_path = home_bin.join("rs-harness");
    std::fs::write(
        &provider_path,
        r#"#!/bin/sh
printf 'pub fn provider_owned() -> usize {
    1
}
'
"#,
    )
    .expect("write provider");
    make_executable(&provider_path);
    write_activation(&root, &[provider("rust", Vec::new())]);

    let output = asp_command(&root)
        .env("PRJ_CACHE_HOME", root.join(".cache"))
        .args([
            "rust",
            "query",
            "--treesitter-query",
            "(function_item name: (identifier) @function.name)",
            "--selector",
            "src/lib.rs:1:3",
            "--code",
        ])
        .output()
        .expect("run asp rust syntax query code");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8(output.stdout).expect("stdout"),
        "pub fn provider_owned() -> usize {\n    1\n}\n"
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn language_facade_query_rejects_syntax_code_output_without_selector() {
    let root = temp_project_root("provider-syntax-query-code-no-selector");
    let bin_dir = root.join(".bin");
    write_echo_provider(&bin_dir, "rs-harness", "rust-provider");
    write_activation(&root, &[provider("rust", Vec::new())]);

    let output = asp_command(&root)
        .env("PRJ_CACHE_HOME", root.join(".cache"))
        .env("PATH", prepend_path(&bin_dir))
        .args([
            "rust",
            "query",
            "--treesitter-query",
            "(function_item name: (identifier) @function.name)",
            "--code",
        ])
        .output()
        .expect("run asp rust syntax query code without selector");

    assert!(
        !output.status.success(),
        "stdout: {}",
        String::from_utf8_lossy(&output.stdout)
    );
    assert!(
        output.stdout.is_empty(),
        "stdout: {}",
        String::from_utf8_lossy(&output.stdout)
    );
    assert!(
        String::from_utf8_lossy(&output.stderr)
            .contains("tree-sitter query --code requires an exact --selector"),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn language_facade_rejects_direct_source_read_code_trailing_root_before_fast_path() {
    let root = temp_project_root("provider-direct-read-code-root-facade");
    let bin_dir = root.join(".bin");
    write_echo_provider(&bin_dir, "rs-harness", "rust-provider");
    write_activation(&root, &[provider("rust", Vec::new())]);
    std::fs::create_dir_all(root.join("src")).expect("create src dir");
    std::fs::write(root.join("src/lib.rs"), "pub fn fast_path() {}\n").expect("write fixture");

    let output = asp_command(&root)
        .env("PRJ_CACHE_HOME", root.join(".cache"))
        .env("PATH", prepend_path(&bin_dir))
        .args([
            "rust",
            "query",
            "--from-hook",
            "direct-source-read",
            "--selector",
            "src/lib.rs:1:1",
            "--code",
            ".",
        ])
        .output()
        .expect("run asp rust direct-source-read code root");

    assert!(
        !output.status.success(),
        "stdout: {}",
        String::from_utf8_lossy(&output.stdout)
    );
    assert!(
        output.stdout.is_empty(),
        "stdout: {}",
        String::from_utf8_lossy(&output.stdout)
    );
    assert!(
        String::from_utf8_lossy(&output.stderr)
            .contains("query/search --code does not accept a trailing PROJECT_ROOT"),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn language_facade_rejects_inline_code_in_compact_frontier_mode() {
    let root = temp_project_root("provider-compact-frontier-inline-code");
    let home_bin = home_local_bin(&root);
    std::fs::create_dir_all(&home_bin).expect("create home local bin");
    let provider_path = home_bin.join("rs-harness");
    std::fs::write(
        &provider_path,
        r#"#!/bin/sh
printf '[read-owner] q=src/lib.rs\n'
printf '|read path=src/lib.rs lineRange=1:2\n'
printf '|code path=src/lib.rs lineRange=1:2 text="pub fn bad() {}"\n'
"#,
    )
    .expect("write provider");
    make_executable(&provider_path);
    write_activation(&root, &[provider("rust", Vec::new())]);

    let output = asp_command(&root)
        .env("PRJ_CACHE_HOME", root.join(".cache"))
        .args([
            "rust",
            "query",
            "--from-hook",
            "direct-source-read",
            "--selector",
            "src/lib.rs:1:2",
            ".",
        ])
        .output()
        .expect("run asp rust compact frontier");

    assert!(
        !output.status.success(),
        "stdout: {}",
        String::from_utf8_lossy(&output.stdout)
    );
    assert!(
        output.stdout.is_empty(),
        "stdout: {}",
        String::from_utf8_lossy(&output.stdout)
    );
    assert!(
        String::from_utf8_lossy(&output.stderr)
            .contains("provider violated ASP compact frontier mode"),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = std::fs::remove_dir_all(root);
}
