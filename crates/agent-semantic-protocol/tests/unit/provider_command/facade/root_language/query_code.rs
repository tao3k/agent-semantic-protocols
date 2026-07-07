use crate::provider_command::support::{
    asp_command, prepend_path, provider, temp_project_root, write_activation, write_echo_provider,
    write_marker_provider,
};

#[test]
fn root_query_facade_owner_code_miss_is_empty_without_provider_fallback() {
    let root = temp_project_root("root-query-owner-code-miss");
    let bin_dir = root.join(".bin");
    let cache_home = root.join(".cache");
    std::fs::create_dir_all(root.join("src")).expect("create src");
    std::fs::write(root.join("src/lib.rs"), "fn demo() {}\n").expect("write rust source");
    write_echo_provider(&bin_dir, "rs-harness", "rs");
    write_activation(&root, &[provider("rust", Vec::new())]);

    let output = asp_command(&root)
        .env("PATH", prepend_path(&bin_dir))
        .env("PRJ_CACHE_HOME", &cache_home)
        .args([
            "query",
            "src/lib.rs",
            "--term",
            "missing_symbol",
            "--workspace",
            ".",
            "--code",
        ])
        .output()
        .expect("run asp query code miss");

    assert!(!output.status.success(), "code miss should fail");
    assert_eq!(String::from_utf8(output.stdout).expect("stdout"), "");
    let stderr = String::from_utf8(output.stderr).expect("stderr");
    assert!(
        stderr.contains("exact selector matched no owner item"),
        "{stderr}"
    );
    assert!(stderr.contains("recommendedNext=asp rust search owner src/lib.rs items --query missing_symbol --workspace . --view seeds"), "{stderr}");
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn root_query_facade_tree_sitter_owner_code_outputs_source_for_non_rust_languages() {
    struct Case {
        language: &'static str,
        provider_binary: &'static str,
        path: &'static str,
        source: &'static str,
        term: &'static str,
        expected: &'static str,
    }

    let cases = [
        Case {
            language: "typescript",
            provider_binary: "ts-harness",
            path: "src/main.ts",
            source: "export function runCli(): number {\n  return 0;\n}\n",
            term: "runCli",
            expected: "export function runCli(): number {\n  return 0;\n}\n",
        },
        Case {
            language: "python",
            provider_binary: "py-harness",
            path: "src/main.py",
            source: "def parse_semantic_search_args():\n    return 1\n",
            term: "parse_semantic_search_args",
            expected: "def parse_semantic_search_args():\n    return 1\n",
        },
        Case {
            language: "julia",
            provider_binary: "asp-julia-harness",
            path: "src/Main.jl",
            source: "function julia_query_owner_items_method_descriptor()\n    1\nend\n",
            term: "julia_query_owner_items_method_descriptor",
            expected: "function julia_query_owner_items_method_descriptor()\n    1\nend\n",
        },
    ];

    for case in cases {
        let root = temp_project_root(&format!("root-query-code-{}", case.language));
        let bin_dir = root.join(".bin");
        let cache_home = root.join(".cache");
        let source_path = root.join(case.path);
        std::fs::create_dir_all(source_path.parent().expect("source parent"))
            .expect("create source dir");
        std::fs::write(&source_path, case.source).expect("write source");
        write_echo_provider(&bin_dir, case.provider_binary, case.language);
        write_activation(&root, &[provider(case.language, Vec::new())]);

        let output = asp_command(&root)
            .env("PATH", prepend_path(&bin_dir))
            .env("PRJ_CACHE_HOME", &cache_home)
            .args([
                "query",
                case.path,
                "--term",
                case.term,
                "--workspace",
                ".",
                "--code",
            ])
            .output()
            .unwrap_or_else(|error| panic!("run asp {} query code: {error}", case.language));

        assert!(
            output.status.success(),
            "{} stderr: {}",
            case.language,
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(
            String::from_utf8(output.stdout).expect("stdout"),
            case.expected,
            "{} stdout",
            case.language
        );
        let _ = std::fs::remove_dir_all(root);
    }
}

#[test]
fn language_facade_python_structural_selector_code_uses_fast_owner_query_without_provider() {
    let root = temp_project_root("python-structural-selector-fast-query");
    let bin_dir = root.join(".bin");
    let cache_home = root.join(".cache");
    let provider_marker = root.join("provider-called");
    std::fs::create_dir_all(root.join("src")).expect("create src");
    let source = "def rank_frontier(graph):\n    return graph\n";
    std::fs::write(root.join("src/ranking.py"), source).expect("write python source");
    write_marker_provider(&bin_dir, "py-harness", &provider_marker);
    write_activation(&root, &[provider("python", Vec::new())]);

    let output = asp_command(&root)
        .env("PATH", prepend_path(&bin_dir))
        .env("PRJ_CACHE_HOME", &cache_home)
        .args([
            "python",
            "query",
            "--selector",
            "python://src/ranking.py#item/function/rank_frontier",
            "--workspace",
            ".",
            "--code",
        ])
        .output()
        .expect("run asp python structural selector query");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(String::from_utf8(output.stdout).expect("stdout"), source);
    assert!(
        !provider_marker.exists(),
        "structural selector query fell through to python provider"
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn root_query_facade_python_owner_code_follows_imported_definition() {
    let root = temp_project_root("root-query-python-import-code");
    let bin_dir = root.join(".bin");
    let cache_home = root.join(".cache");
    std::fs::create_dir_all(root.join("src/pkg")).expect("create package");
    std::fs::write(
        root.join("src/pkg/_cli_args.py"),
        "def call(args):\n    from ._semantic_search_cli import parse_semantic_search_args\n    return parse_semantic_search_args(args)\n",
    )
    .expect("write owner");
    let target_source = "def parse_semantic_search_args(args):\n    return args\n";
    std::fs::write(root.join("src/pkg/_semantic_search_cli.py"), target_source)
        .expect("write target");
    write_echo_provider(&bin_dir, "py-harness", "python");
    write_activation(&root, &[provider("python", Vec::new())]);

    let output = asp_command(&root)
        .env("PATH", prepend_path(&bin_dir))
        .env("PRJ_CACHE_HOME", &cache_home)
        .args([
            "query",
            "src/pkg/_cli_args.py",
            "--term",
            "parse_semantic_search_args",
            "--workspace",
            ".",
            "--code",
        ])
        .output()
        .expect("run asp python imported query code");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8(output.stdout).expect("stdout"),
        target_source
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn root_query_facade_tree_sitter_owner_code_miss_is_empty() {
    let root = temp_project_root("root-query-tree-sitter-code-miss");
    let bin_dir = root.join(".bin");
    let cache_home = root.join(".cache");
    std::fs::create_dir_all(root.join("src")).expect("create src");
    std::fs::write(root.join("src/main.ts"), "export function runCli() {}\n")
        .expect("write source");
    write_echo_provider(&bin_dir, "ts-harness", "typescript");
    write_activation(&root, &[provider("typescript", Vec::new())]);

    let output = asp_command(&root)
        .env("PATH", prepend_path(&bin_dir))
        .env("PRJ_CACHE_HOME", &cache_home)
        .args([
            "query",
            "src/main.ts",
            "--term",
            "missingSymbol",
            "--workspace",
            ".",
            "--code",
        ])
        .output()
        .expect("run asp typescript query code miss");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(String::from_utf8(output.stdout).expect("stdout"), "");
    let _ = std::fs::remove_dir_all(root);
}
