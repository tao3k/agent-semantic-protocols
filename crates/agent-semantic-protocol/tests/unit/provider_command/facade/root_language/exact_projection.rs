use crate::provider_command::support::{
    asp_command, provider, temp_project_root, write_activation, write_marker_provider,
    write_stdout_stderr_provider,
};

#[test]
fn root_query_facade_exact_selector_fails_closed_on_empty_provider_packet() {
    let root = temp_project_root("root-query-owner-code-miss");
    let bin_dir = root.join(".bin");
    let provider_marker = root.join("provider-called");
    std::fs::create_dir_all(root.join("src")).expect("create src");
    std::fs::write(root.join("src/lib.rs"), "fn demo() {}\n").expect("write rust source");
    write_marker_provider(&bin_dir, "rs-harness", &provider_marker);
    write_activation(&root, &[provider("rust", Vec::new())]);

    let output = asp_command(&root)
        .args([
            "query",
            "--language",
            "rust",
            "--selector",
            "rust://src/lib.rs#item/function/missing_symbol",
            "--workspace",
            ".",
            "--projection",
            "source",
        ])
        .output()
        .expect("run asp exact projection miss");

    assert!(
        !output.status.success(),
        "empty exact-selector packet must fail"
    );
    assert_eq!(String::from_utf8(output.stdout).expect("stdout"), "");
    let stderr = String::from_utf8(output.stderr).expect("stderr");
    assert!(
        stderr.contains("failed to decode exact-selector provider packet"),
        "{stderr}"
    );
    assert!(
        provider_marker.exists(),
        "exact selector did not reach provider"
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn root_query_facade_non_rust_exact_selectors_are_provider_owned_and_fail_closed() {
    struct Case {
        language: &'static str,
        provider_binary: &'static str,
        path: &'static str,
        source: &'static str,
        term: &'static str,
    }

    let cases = [
        Case {
            language: "typescript",
            provider_binary: "asp-typescript",
            path: "src/main.ts",
            source: "export function runCli(): number {\n  return 0;\n}\n",
            term: "runCli",
        },
        Case {
            language: "python",
            provider_binary: "py-harness",
            path: "src/main.py",
            source: "def parse_semantic_search_args():\n    return 1\n",
            term: "parse_semantic_search_args",
        },
        Case {
            language: "julia",
            provider_binary: "asp-julia-harness",
            path: "src/Main.jl",
            source: "function julia_query_owner_items_method_descriptor()\n    1\nend\n",
            term: "julia_query_owner_items_method_descriptor",
        },
    ];

    for case in cases {
        let root = temp_project_root(&format!("root-query-code-{}", case.language));
        let bin_dir = root.join(".bin");
        let provider_marker = root.join("provider-called");
        let source_path = root.join(case.path);
        std::fs::create_dir_all(source_path.parent().expect("source parent"))
            .expect("create source dir");
        std::fs::write(&source_path, case.source).expect("write source");
        write_marker_provider(&bin_dir, case.provider_binary, &provider_marker);
        write_activation(&root, &[provider(case.language, Vec::new())]);
        let selector = format!(
            "{}://{}#item/function/{}",
            case.language, case.path, case.term
        );

        let output = asp_command(&root)
            .args([
                "query",
                "--language",
                case.language,
                "--selector",
                &selector,
                "--workspace",
                ".",
                "--projection",
                "source",
            ])
            .output()
            .unwrap_or_else(|error| panic!("run asp {} exact projection: {error}", case.language));

        assert!(
            !output.status.success(),
            "{} should fail closed",
            case.language
        );
        let stderr = String::from_utf8(output.stderr).expect("stderr");
        assert!(
            stderr.contains("failed to decode exact-selector provider packet"),
            "{} stderr: {stderr}",
            case.language
        );
        assert!(
            provider_marker.exists(),
            "{} exact selector did not reach provider",
            case.language
        );
        let _ = std::fs::remove_dir_all(root);
    }
}

#[test]
fn language_facade_python_exact_selector_is_provider_owned_and_fails_closed() {
    let root = temp_project_root("python-structural-selector-fast-query");
    let bin_dir = root.join(".bin");
    let provider_marker = root.join("provider-called");
    std::fs::create_dir_all(root.join("src")).expect("create src");
    let source = "def rank_frontier(graph):\n    return graph\n";
    std::fs::write(root.join("src/ranking.py"), source).expect("write python source");
    write_marker_provider(&bin_dir, "py-harness", &provider_marker);
    write_activation(&root, &[provider("python", Vec::new())]);

    let output = asp_command(&root)
        .args([
            "python",
            "query",
            "--selector",
            "python://src/ranking.py#item/function/rank_frontier",
            "--workspace",
            ".",
            "--projection",
            "source",
        ])
        .output()
        .expect("run asp python structural selector query");

    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).expect("stderr");
    assert!(
        stderr.contains("failed to decode exact-selector provider packet"),
        "{stderr}"
    );
    assert!(
        provider_marker.exists(),
        "exact selector did not reach python provider"
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn root_search_facade_python_import_reasoning_stays_in_search() {
    let root = temp_project_root("root-query-python-import-code");
    let bin_dir = root.join(".bin");
    std::fs::create_dir_all(root.join("src/pkg")).expect("create package");
    std::fs::write(
        root.join("src/pkg/_cli_args.py"),
        "def call(args):\n    from ._semantic_search_cli import parse_semantic_search_args\n    return parse_semantic_search_args(args)\n",
    )
    .expect("write owner");
    std::fs::write(
        root.join("src/pkg/_semantic_search_cli.py"),
        "def parse_semantic_search_args(args):\n    return args\n",
    )
    .expect("write target");
    write_stdout_stderr_provider(
        &bin_dir,
        "py-harness",
        "[search-owner] q=src/pkg/_cli_args.py pkg=. selector=items alg=item-frontier\n\
O=owner:path(src/pkg/_cli_args.py)!owner;I=item:symbol(parse_semantic_search_args)!syntax\n\
O>{I:contains}\n\
rank=I,O frontier=I.syntax\n",
        "",
    );
    write_activation(&root, &[provider("python", Vec::new())]);

    let output = asp_command(&root)
        .args([
            "search",
            "--language",
            "python",
            "owner",
            "src/pkg/_cli_args.py",
            "items",
            "--query",
            "parse_semantic_search_args",
            "--workspace",
            ".",
            "--view",
            "seeds",
        ])
        .output()
        .expect("run asp python import reasoning search");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("stdout");
    assert!(stdout.contains("[search-owner]"), "{stdout}");
    assert!(
        stdout.contains("item:symbol(parse_semantic_search_args)"),
        "{stdout}"
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn root_query_facade_typescript_exact_selector_fails_closed_on_empty_packet() {
    let root = temp_project_root("root-query-tree-sitter-code-miss");
    let bin_dir = root.join(".bin");
    let provider_marker = root.join("provider-called");
    std::fs::create_dir_all(root.join("src")).expect("create src");
    std::fs::write(root.join("src/main.ts"), "export function runCli() {}\n")
        .expect("write source");
    write_marker_provider(&bin_dir, "asp-typescript", &provider_marker);
    write_activation(&root, &[provider("typescript", Vec::new())]);

    let output = asp_command(&root)
        .args([
            "query",
            "--language",
            "typescript",
            "--selector",
            "typescript://src/main.ts#item/function/missingSymbol",
            "--workspace",
            ".",
            "--projection",
            "source",
        ])
        .output()
        .expect("run asp typescript exact projection miss");

    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).expect("stderr");
    assert!(
        stderr.contains("failed to decode exact-selector provider packet"),
        "{stderr}"
    );
    assert!(
        provider_marker.exists(),
        "exact selector did not reach TypeScript provider"
    );
    let _ = std::fs::remove_dir_all(root);
}
