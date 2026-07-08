use super::support;

fn direct_read_fixture(name: &str) -> std::path::PathBuf {
    let root = support::temp_project_root(name);
    std::fs::create_dir_all(root.join("src")).expect("create src dir");
    let source = (1..=50)
        .map(|line| format!("pub fn line_{line}() {{}}\n"))
        .collect::<String>();
    std::fs::write(root.join("src/lib.rs"), source).expect("write source");
    support::write_activation(&root, &[support::provider("rust", Vec::new())]);
    root
}

#[test]
fn direct_source_read_rejects_missing_fallback_reason() {
    let root = direct_read_fixture("direct-read-whole-file");
    let output = support::asp_command(&root)
        .args([
            "rust",
            "query",
            "--from-hook",
            "direct-source-read",
            "--selector",
            "src/lib.rs",
            "--workspace",
            ".",
            "--code",
        ])
        .output()
        .expect("run asp");

    assert!(
        !output.status.success(),
        "stdout={}",
        String::from_utf8_lossy(&output.stdout)
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("direct-source-read requires --fallback-reason"),
        "stderr={stderr}"
    );
    std::fs::remove_dir_all(root).expect("remove temp root");
}

#[test]
fn direct_source_read_rejects_whole_file_selector_even_with_fallback_reason() {
    let root = direct_read_fixture("direct-read-whole-file-with-reason");
    let output = support::asp_command(&root)
        .args([
            "rust",
            "query",
            "--from-hook",
            "direct-source-read",
            "--fallback-reason",
            "parser-missing-structural-selector",
            "--selector",
            "src/lib.rs",
            "--workspace",
            ".",
            "--code",
        ])
        .output()
        .expect("run asp");

    assert!(
        !output.status.success(),
        "stdout={}",
        String::from_utf8_lossy(&output.stdout)
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("whole-file fallback is disabled"),
        "stderr={stderr}"
    );
    std::fs::remove_dir_all(root).expect("remove temp root");
}

#[test]
fn query_file_selector_recovery_is_language_neutral() {
    let root = support::temp_project_root("query-code-file-selector-recovery");
    let language_cases = [
        ("rust", "src/lib.rs"),
        ("typescript", "src/index.ts"),
        ("python", "src/main.py"),
        ("julia", "src/Main.jl"),
        ("gerbil-scheme", "src/main.ss"),
    ];
    std::fs::create_dir_all(root.join("src")).expect("create src dir");
    for (_, selector) in language_cases {
        std::fs::write(root.join(selector), "placeholder\n").expect("write source fixture");
    }
    let providers = language_cases
        .iter()
        .map(|(language_id, _)| support::provider(*language_id, Vec::new()))
        .collect::<Vec<_>>();
    support::write_activation(&root, &providers);

    for (language_id, selector) in language_cases {
        for projection_args in [vec!["--code"], vec!["--names-only"], Vec::new()] {
            let mut args = vec![
                language_id,
                "query",
                "--selector",
                selector,
                "--workspace",
                ".",
            ];
            args.extend(projection_args);
            let output = support::asp_command(&root)
                .args(args)
                .output()
                .expect("run asp");

            assert!(
                !output.status.success(),
                "language={language_id} stdout={}",
                String::from_utf8_lossy(&output.stdout)
            );
            let stderr = String::from_utf8_lossy(&output.stderr);
            assert!(
                stderr.contains("selectorState=file-selector"),
                "language={language_id} stderr={stderr}"
            );
            assert!(stderr.contains("projection=query"), "stderr={stderr}");
            assert!(stderr.contains("allowed=false"), "stderr={stderr}");
            assert!(
                stderr.contains("reason=file-selectors-are-not-query-selectors"),
                "stderr={stderr}"
            );
            assert!(
                stderr.contains("nextAction=materialize-owner-items"),
                "stderr={stderr}"
            );
            assert!(
                stderr.contains(&format!(
                    "nextCommand=asp {language_id} search owner {selector} items --workspace . --view seeds"
                )),
                "stderr={stderr}"
            );
            assert!(
                stderr.contains(&format!(
                    "requiredSelector={language_id}://{selector}#item/<kind>/<name>"
                )),
                "stderr={stderr}"
            );
        }
    }

    std::fs::remove_dir_all(root).expect("remove temp root");
}

#[test]
fn direct_source_read_rejects_wide_selector_range() {
    let root = direct_read_fixture("direct-read-wide-range");
    let output = support::asp_command(&root)
        .args([
            "rust",
            "query",
            "--from-hook",
            "direct-source-read",
            "--fallback-reason",
            "parser-missing-structural-selector",
            "--selector",
            "src/lib.rs:1-41",
            "--workspace",
            ".",
            "--code",
        ])
        .output()
        .expect("run asp");

    assert!(
        !output.status.success(),
        "stdout={}",
        String::from_utf8_lossy(&output.stdout)
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("exceeds max 24"), "stderr={stderr}");
    std::fs::remove_dir_all(root).expect("remove temp root");
}

#[test]
fn direct_source_read_accepts_bounded_selector_range() {
    let root = direct_read_fixture("direct-read-bounded-range");
    let output = support::asp_command(&root)
        .args([
            "rust",
            "query",
            "--from-hook",
            "direct-source-read",
            "--fallback-reason",
            "parser-missing-structural-selector",
            "--selector",
            "src/lib.rs:2-3",
            "--workspace",
            ".",
            "--code",
        ])
        .output()
        .expect("run asp");

    assert!(
        output.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("stdout");
    assert_eq!(stdout, "pub fn line_2() {}\npub fn line_3() {}\n");
    std::fs::remove_dir_all(root).expect("remove temp root");
}

#[test]
fn ordinary_selector_query_does_not_use_direct_source_read_limit() {
    let root = direct_read_fixture("ordinary-selector-query");
    let bin_dir = root.join(".bin");
    support::write_stdout_stderr_exit_provider(
        &bin_dir,
        "rs-harness",
        &std::fs::read_to_string(root.join("src/lib.rs")).expect("read source"),
        "",
        0,
    );
    let output = support::asp_command(&root)
        .env("PATH", support::prepend_path(&bin_dir))
        .args([
            "rust",
            "query",
            "--selector",
            "src/lib.rs:1-41",
            "--workspace",
            ".",
            "--code",
        ])
        .output()
        .expect("run asp");

    assert!(
        output.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("stdout");
    assert!(stdout.contains("pub fn line_1() {}"), "stdout={stdout}");
    assert!(stdout.contains("pub fn line_41() {}"), "stdout={stdout}");
    std::fs::remove_dir_all(root).expect("remove temp root");
}
