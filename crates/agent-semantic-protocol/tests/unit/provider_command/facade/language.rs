use std::env;
use std::path::PathBuf;

use crate::provider_command::support::{
    asp_command, prepend_path, provider, temp_project_root, write_activation,
    write_echo_provider, write_pwd_provider,
};

#[test]
fn language_facade_discovers_activation_from_child_directory() {
    let root = temp_project_root("child-search-facade");
    let bin_dir = root.join(".bin");
    let cache_home = root.join(".cache");
    let child_dir = root.join("nested").join("workspace");
    std::fs::create_dir_all(&child_dir).expect("create child directory");
    write_pwd_provider(&bin_dir, "rs-harness");
    write_activation(&root, &[provider("rust", Vec::new())]);
    let root_arg = root.to_str().expect("utf8 root").to_string();

    let output = asp_command(&root)
        .current_dir(&child_dir)
        .env("PATH", prepend_path(&bin_dir))
        .env("PRJ_CACHE_HOME", &cache_home)
        .args([
            "rust",
            "agent",
            "doctor",
            "--workspace",
            root_arg.as_str(),
            "--json",
        ])
        .output()
        .expect("run asp rust agent doctor");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let actual_root = PathBuf::from(String::from_utf8(output.stdout).expect("stdout").trim());
    assert_eq!(
        std::fs::canonicalize(actual_root).expect("canonical actual root"),
        std::fs::canonicalize(&root).expect("canonical expected root")
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn language_facade_version_does_not_require_activation() {
    let root = temp_project_root("language-version-without-activation");

    for arg in ["--version", "version"] {
        let output = asp_command(&root)
            .args(["rust", arg])
            .output()
            .expect("run asp rust version");

        assert!(
            output.status.success(),
            "arg={arg} stderr={}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(
            String::from_utf8(output.stdout).expect("stdout"),
            format!("asp {}\n", env!("CARGO_PKG_VERSION"))
        );
        assert!(output.stderr.is_empty());
    }
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn language_facade_rejects_unsupported_language_without_unrelated_provider_recovery() {
    let root = temp_project_root("language-unsupported-facade");
    write_activation(
        &root,
        &[
            provider("gerbil-scheme", Vec::new()),
            provider("rust", Vec::new()),
            provider("typescript", Vec::new()),
        ],
    );

    let output = asp_command(&root)
        .args([
            "scheme",
            "search",
            "lexical",
            "demo",
            "owner",
            "tests",
            "--workspace",
            ".",
            "--view",
            "seeds",
        ])
        .output()
        .expect("run unsupported language facade");

    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).expect("stderr");
    assert!(
        stderr.contains("unsupported ASP language facade `scheme`"),
        "{stderr}"
    );
    assert!(
        stderr
            .contains("Known language facades: gerbil-scheme|julia|md|org|python|rust|typescript."),
        "{stderr}"
    );
    assert!(stderr.contains("asp providers"), "{stderr}");
    assert!(
        stderr.contains("Do not switch to an unrelated active facade"),
        "{stderr}"
    );
    assert!(
        !stderr.contains("asp typescript search lexical"),
        "{stderr}"
    );
    assert!(!stderr.contains("Suggested matching facade"), "{stderr}");
    assert!(!stderr.contains("asp gerbil-scheme search"), "{stderr}");
    let _ = std::fs::remove_dir_all(root);
}


#[test]
fn language_facade_normalizes_relative_nested_project_root_arg() {
    let root = temp_project_root("relative-child-package-search-facade");
    let child_dir = root.join("languages").join("rust-lang-project-harness");
    std::fs::create_dir_all(&child_dir).expect("create child dir");
    std::fs::write(
        child_dir.join("Cargo.toml"),
        "[package]\nname = \"nested-rust-lang-project-harness\"\nversion = \"0.1.0\"\n",
    )
    .expect("write child manifest");
    std::fs::create_dir_all(child_dir.join("src")).expect("create child source dir");
    std::fs::write(
        child_dir.join("src").join("lib.rs"),
        "fn nested_workspace_owner() {}\n",
    )
        .expect("write child source");
    let bin_dir = root.join(".bin");
    let cache_home = root.join(".cache");
    write_pwd_provider(&bin_dir, "rs-harness");
    write_activation(&root, &[provider("rust", Vec::new())]);

    let output = asp_command(&root)
        .current_dir(&root)
        .env("PRJ_CACHE_HOME", &cache_home)
        .args([
            "rust",
            "search",
            "--treesitter-query",
            "(function_item name: (identifier) @function.name)",
            "--workspace",
            "languages/rust-lang-project-harness",
            "--json",
        ])
        .output()
        .expect("run search facade");
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let receipt: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("tree-sitter search JSON");
    let canonical_child_dir =
        std::fs::canonicalize(&child_dir).expect("canonicalize nested workspace");
    let expected_selector = format!("workspace:{}", canonical_child_dir.display());
    assert_eq!(
        receipt["selector"].as_str(),
        Some(expected_selector.as_str()),
        "{receipt}"
    );
    let native_fact_refs = receipt["nativeFactRefs"]
        .as_array()
        .expect("native fact refs");
    assert!(
        native_fact_refs
            .iter()
            .filter_map(serde_json::Value::as_str)
            .any(|fact| fact.contains(":syntax:src/lib.rs:")),
        "tree-sitter facts must be rooted in the selected nested workspace: receipt={receipt}"
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn language_facade_selects_matching_provider_from_activation() {
    let root = temp_project_root("typescript-search-facade");
    let bin_dir = root.join(".bin");
    let cache_home = root.join(".cache");
    write_echo_provider(&bin_dir, "rs-harness", "rs");
    write_echo_provider(&bin_dir, "ts-harness", "ts");
    write_activation(
        &root,
        &[
            provider("rust", Vec::new()),
            provider("typescript", Vec::new()),
        ],
    );

    let output = asp_command(&root)
        .env("PATH", prepend_path(&bin_dir))
        .env("PRJ_CACHE_HOME", &cache_home)
        .args(["typescript", "check", "--changed", "."])
        .output()
        .expect("run asp typescript check");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8(output.stdout).expect("stdout"),
        "ts args=[check][--changed]\n"
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn registered_language_search_pipe_non_executable_selectors_do_not_emit_actions() {
    let root = temp_project_root("registered-language-selector-gate");
    let bin_dir = root.join(".bin");
    let cache_home = root.join(".cache");
    let marker = root.join("provider-called");
    crate::provider_command::support::write_marker_provider(&bin_dir, "rs-harness", &marker);
    crate::provider_command::support::write_marker_provider(&bin_dir, "ts-harness", &marker);
    let language_cases = [
        (
            "rust",
            "src/lib.rs:1:5",
            "rust://src/lib.rs#item/symbol/vec",
        ),
        (
            "typescript",
            "src/index.ts:1:5",
            "typescript://src/index.ts#item/symbol/render",
        ),
        (
            "python",
            "src/main.py:1:5",
            "python://src/main.py#item/symbol/render",
        ),
        (
            "julia",
            "src/Main.jl:1:5",
            "julia://src/Main.jl#item/symbol/render",
        ),
        (
            "gerbil-scheme",
            "src/main.ss:1:5",
            "gerbil-scheme://src/main.ss#item/symbol/render",
        ),
        (
            "org",
            "docs/index.org:1:5",
            "org://docs/index.org#item/symbol/render",
        ),
        ("md", "README.md:1:5", "md://README.md#item/symbol/render"),
    ];
    let providers = language_cases
        .iter()
        .map(|(language_id, _, _)| provider(*language_id, Vec::new()))
        .collect::<Vec<_>>();
    write_activation(&root, &providers);

    for (language_id, file_range_selector, symbol_selector) in language_cases {
        for (selector, executable) in [(file_range_selector, false), (symbol_selector, true)] {
            let output = asp_command(&root)
                .env("PATH", prepend_path(&bin_dir))
                .env("PRJ_CACHE_HOME", &cache_home)
                .args([
                    language_id,
                    "search",
                    "pipe",
                    "--selector",
                    selector,
                    "--query",
                    "selector identity drift",
                    "--workspace",
                    ".",
                    "--view",
                    "seeds",
                ])
                .output()
                .expect("run registered language search pipe selector gate");

            assert!(
                output.status.success(),
                "language={language_id} selector={selector} stderr={}",
                String::from_utf8_lossy(&output.stderr)
            );
            let stdout = String::from_utf8(output.stdout).expect("stdout");
            assert!(stdout.contains("source=selector"), "{stdout}");
            assert!(
                stdout.contains(&format!("selectorSeed={selector}")),
                "{stdout}"
            );
            if executable {
                assert!(
                    stdout.contains("actionFrontier=A1.query-code,A2.owner-items"),
                    "language={language_id} selector={selector} stdout={stdout}"
                );
                assert!(
                    stdout.contains("recommendedNext=A1.query-code"),
                    "language={language_id} selector={selector} stdout={stdout}"
                );
            } else {
                assert!(
                    !stdout.contains("query-code"),
                    "language={language_id} selector={selector} stdout={stdout}"
                );
                assert!(
                    stdout.lines().any(|line| line == "actionFrontier="),
                    "language={language_id} selector={selector} stdout={stdout}"
                );
                assert!(
                    stdout.lines().any(|line| line == "recommendedNext=-"),
                    "language={language_id} selector={selector} stdout={stdout}"
                );
            }
        }
    }
    assert!(
        !marker.exists(),
        "registered-language selector gate must not spawn providers"
    );
    let _ = std::fs::remove_dir_all(root);
}
