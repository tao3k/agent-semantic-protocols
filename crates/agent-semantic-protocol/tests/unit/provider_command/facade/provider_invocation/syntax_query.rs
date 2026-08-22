use crate::provider_command::support::{
    asp_command, make_executable, prepend_path, provider, state_runtime_bin, temp_project_root,
    write_activation, write_echo_provider, write_rust_owner_frontier_provider,
};

#[test]
fn language_facade_query_passes_tree_sitter_query_and_exact_selector_to_each_provider() {
    let root = temp_project_root("provider-syntax-query-plan-facade");
    let bin_dir = root.join(".bin");
    write_echo_provider(&bin_dir, "rs-harness", "rust-provider");
    write_echo_provider(&bin_dir, "asp-typescript", "typescript-provider");
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
    }

    let cases = [
        SyntaxQueryCase {
            language: "rust",
            label: "rust-provider",
            query: "(function_item name: (identifier) @function.name (#eq? @function.name \"parse_query\"))",
            selector: "src/cli/query.rs",
        },
        SyntaxQueryCase {
            language: "typescript",
            label: "typescript-provider",
            query: "(function_declaration name: (identifier) @function.name (#eq? @function.name \"parseTreeSitterQueryArgs\"))",
            selector: "src/cli/protocol-tree-sitter-query.ts",
        },
        SyntaxQueryCase {
            language: "python",
            label: "python-provider",
            query: "(function_definition name: (identifier) @function.name (#eq? @function.name \"run_query_command\"))",
            selector: "src/python_lang_project_harness/_cli_query.py",
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
        assert!(!stdout.contains("--asp-syntax-query-"), "stdout: {stdout}");
    }
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn language_facade_query_allows_syntax_code_output_with_exact_selector() {
    let root = temp_project_root("provider-syntax-query-stdout-facade");
    let home_bin = state_runtime_bin(&root);
    std::fs::create_dir_all(&home_bin).expect("create state-home runtime bin");
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
fn language_facade_treesitter_file_selector_code_stays_provider_owned() {
    let root = temp_project_root("provider-syntax-query-file-selector-code");
    let home_bin = state_runtime_bin(&root);
    std::fs::create_dir_all(&home_bin).expect("create state-home runtime bin");
    let provider_path = home_bin.join("rs-harness");
    std::fs::write(
        &provider_path,
        r#"#!/bin/sh
printf 'provider syntax query output
'
"#,
    )
    .expect("write provider");
    make_executable(&provider_path);
    write_activation(&root, &[provider("rust", Vec::new())]);
    std::fs::create_dir_all(root.join("src")).expect("create src dir");
    std::fs::write(root.join("src/lib.rs"), "pub fn local_source() {}\n").expect("write source");

    let output = asp_command(&root)
        .env("PRJ_CACHE_HOME", root.join(".cache"))
        .args([
            "rust",
            "query",
            "--treesitter-query",
            "(function_item name: (identifier) @function.name)",
            "--selector",
            "src/lib.rs",
            "--code",
        ])
        .output()
        .expect("run asp rust syntax query code");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("stdout");
    assert_eq!(stdout, "provider syntax query output\n");
    assert!(!stdout.contains("local_source"), "{stdout}");
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn language_facade_query_file_selector_code_is_rejected() {
    let root = temp_project_root("provider-query-file-selector-code-source");
    write_rust_owner_frontier_provider(&root);
    write_activation(&root, &[provider("rust", Vec::new())]);
    std::fs::create_dir_all(root.join("src")).expect("create src dir");
    let source = "pub struct QueryExpr;\n\npub fn parse_query_expr() {}\n";
    std::fs::write(root.join("src/core.rs"), source).expect("write fixture");

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
        .expect("run asp rust owner selector query");

    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).expect("stderr");
    assert!(
        stderr.contains("invalid query selector `src/core.rs`"),
        "{stderr}"
    );
    assert!(
        stderr.contains("exact parser-owned item selector"),
        "{stderr}"
    );
    assert!(!stderr.contains("direct-source-read"), "{stderr}");
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn registered_language_facade_query_source_selector_code_is_rejected() {
    let source_language_cases = agent_semantic_hook::builtin_provider_manifests()
        .into_iter()
        .filter(|manifest| {
            manifest.execution() == agent_semantic_hook::ProviderExecution::ExternalProcess
        })
        .filter_map(|manifest| {
            let extension = manifest.document_resolution()?.extensions.first()?.clone();
            let selector = format!("src/core{}", extension);
            Some((manifest.language_id().clone(), selector))
        })
        .collect::<Vec<_>>();
    assert!(
        !source_language_cases.is_empty(),
        "registered provider manifests must include source-language facades"
    );

    for (language_id, selector) in source_language_cases {
        let root = temp_project_root(&format!(
            "provider-query-registered-file-selector-code-source-{language_id}"
        ));
        write_activation(&root, &[provider(language_id.clone(), Vec::new())]);
        std::fs::create_dir_all(root.join("src")).expect("create src dir");
        std::fs::write(root.join(&selector), "fixture source\n").expect("write fixture");

        let output = asp_command(&root)
            .env("PRJ_CACHE_HOME", root.join(".cache"))
            .args([
                language_id.as_str(),
                "query",
                "--selector",
                selector.as_str(),
                "--workspace",
                ".",
                "--code",
            ])
            .output()
            .expect("run asp registered language selector query");

        assert!(
            !output.status.success(),
            "{language_id} unexpectedly succeeded"
        );
        let stderr = String::from_utf8(output.stderr).expect("stderr");
        assert!(
            stderr.contains(&format!("invalid query selector `{selector}`")),
            "{language_id}: {stderr}"
        );
        assert!(
            stderr.contains(&format!("{language_id}://"))
                && stderr.contains("#item/<kind>/<symbol>"),
            "{language_id}: {stderr}"
        );
        assert!(
            !stderr.contains("direct-source-read"),
            "{language_id}: {stderr}"
        );
        let _ = std::fs::remove_dir_all(root);
    }
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
            .contains("workspace Tree-sitter discovery is search-owned"),
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
fn language_facade_accepts_bounded_read_owner_projection_for_direct_source_read() {
    let root = temp_project_root("provider-compact-frontier-inline-code");
    let home_bin = state_runtime_bin(&root);
    std::fs::create_dir_all(&home_bin).expect("create state-home runtime bin");
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
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("stdout");
    assert!(stdout.contains("[read-owner] q=src/lib.rs"), "{stdout}");
    assert!(
        stdout.contains("|read path=src/lib.rs lineRange=1:2"),
        "{stdout}"
    );
    assert!(
        stdout.contains("|code path=src/lib.rs lineRange=1:2"),
        "{stdout}"
    );
    let _ = std::fs::remove_dir_all(root);
}
