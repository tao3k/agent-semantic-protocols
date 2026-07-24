use crate::provider_command::support;

#[test]
fn query_bare_file_selector_code_returns_source_not_owner_frontier() {
    let root = support::temp_project_root("query-bare-file-selector-code");
    let bin_dir = root.join(".bin");
    let marker = root.join("provider-called");
    std::fs::create_dir_all(root.join("src")).expect("create src");
    std::fs::write(
        root.join("src/lib.rs"),
        "pub fn direct_code_fixture() {}\npub fn unrelated() {}\n",
    )
    .expect("write source");
    support::write_marker_provider(&bin_dir, "rs-harness", &marker);
    support::write_activation(&root, &[support::provider("rust", Vec::new())]);

    let output = support::asp_command(&root)
        .env("PATH", support::prepend_path(&bin_dir))
        .env("PRJ_CACHE_HOME", root.join(".cache"))
        .args([
            "rust",
            "query",
            "--selector",
            "src/lib.rs",
            "--workspace",
            ".",
            "--code",
        ])
        .output()
        .expect("run asp rust query selector code");

    assert!(
        !output.status.success(),
        "stdout: {}",
        String::from_utf8_lossy(&output.stdout)
    );
    let stderr = String::from_utf8(output.stderr).expect("stderr");
    assert!(
        stderr.contains("file selectors are not executable query selectors"),
        "{stderr}"
    );
    assert!(stderr.contains("search owner <path> items"), "{stderr}");
    assert!(
        !marker.exists(),
        "bare selector code query should not spawn provider"
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn query_structural_item_selector_code_returns_item_code() {
    let root = support::temp_project_root("query-structural-item-selector-code");
    let bin_dir = root.join(".bin");
    let marker = root.join("provider-called");
    std::fs::create_dir_all(root.join("src")).expect("create src");
    std::fs::write(
        root.join("src/lib.rs"),
        "pub fn direct_code_fixture() {\n    let value = 1;\n}\npub fn unrelated() {}\n",
    )
    .expect("write source");
    support::write_marker_provider(&bin_dir, "rs-harness", &marker);
    support::write_activation(&root, &[support::provider("rust", Vec::new())]);

    let output = support::asp_command(&root)
        .env("PATH", support::prepend_path(&bin_dir))
        .env("PRJ_CACHE_HOME", root.join(".cache"))
        .args([
            "rust",
            "query",
            "--selector",
            "rust://src/lib.rs#item/fn/direct_code_fixture",
            "--workspace",
            ".",
            "--code",
        ])
        .output()
        .expect("run asp rust query structural selector code");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("stdout");
    assert_eq!(
        stdout,
        "pub fn direct_code_fixture() {\n    let value = 1;\n}\n"
    );
    assert!(!stdout.contains("[search-owner]"), "{stdout}");
    assert!(
        !marker.exists(),
        "structural selector code query should not spawn provider"
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn query_structural_impl_selector_code_returns_impl_block() {
    let root = support::temp_project_root("query-structural-impl-selector-code");
    let bin_dir = root.join(".bin");
    let marker = root.join("provider-called");
    std::fs::create_dir_all(root.join("src")).expect("create src");
    std::fs::write(
        root.join("src/lib.rs"),
        "pub struct AspSessionPolicy;\n\
         impl AspSessionPolicy {\n\
             fn main_asp_command_allowed(&self) -> bool {\n\
                 true\n\
             }\n\
         }\n",
    )
    .expect("write source");
    support::write_marker_provider(&bin_dir, "rs-harness", &marker);
    support::write_activation(&root, &[support::provider("rust", Vec::new())]);

    let output = support::asp_command(&root)
        .env("PATH", support::prepend_path(&bin_dir))
        .env("PRJ_CACHE_HOME", root.join(".cache"))
        .args([
            "rust",
            "query",
            "--selector",
            "rust://src/lib.rs#item/impl/AspSessionPolicy",
            "--workspace",
            ".",
            "--code",
        ])
        .output()
        .expect("run asp rust query structural impl selector code");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("stdout");
    assert!(stdout.starts_with("impl AspSessionPolicy {"), "{stdout}");
    assert!(
        stdout.contains("fn main_asp_command_allowed(&self) -> bool"),
        "{stdout}"
    );
    assert!(!stdout.contains("[search-owner]"), "{stdout}");
    assert!(
        !marker.exists(),
        "structural selector impl code query should not spawn provider"
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn query_structural_function_selector_code_resolves_rust_method() {
    let root = support::temp_project_root("query-structural-function-selector-code-method");
    let bin_dir = root.join(".bin");
    let marker = root.join("provider-called");
    std::fs::create_dir_all(root.join("src")).expect("create src");
    std::fs::write(
        root.join("src/lib.rs"),
        "pub struct AspSessionPolicy;\n\
         impl AspSessionPolicy {\n\
             fn main_asp_command_allowed(&self) -> bool {\n\
                 true\n\
             }\n\
         }\n",
    )
    .expect("write source");
    support::write_marker_provider(&bin_dir, "rs-harness", &marker);
    support::write_activation(&root, &[support::provider("rust", Vec::new())]);

    let output = support::asp_command(&root)
        .env("PATH", support::prepend_path(&bin_dir))
        .env("PRJ_CACHE_HOME", root.join(".cache"))
        .env("CODEX_THREAD_ID", "test-agent-platform")
        .args([
            "rust",
            "query",
            "--selector",
            "rust://src/lib.rs#item/function/main_asp_command_allowed",
            "--workspace",
            ".",
            "--code",
        ])
        .output()
        .expect("run asp rust query structural function selector code");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("stdout");
    assert_eq!(
        stdout,
        "fn main_asp_command_allowed(&self) -> bool {\ntrue\n}\n"
    );
    assert!(!stdout.contains("[search-owner]"), "{stdout}");
    assert!(
        !marker.exists(),
        "structural selector code query should not spawn provider"
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn query_structural_item_selector_code_miss_fails_instead_of_empty_success() {
    let root = support::temp_project_root("query-structural-item-selector-code-miss");
    let bin_dir = root.join(".bin");
    let marker = root.join("provider-called");
    std::fs::create_dir_all(root.join("src")).expect("create src");
    std::fs::write(root.join("src/lib.rs"), "pub fn direct_code_fixture() {}\n")
        .expect("write source");
    support::write_marker_provider(&bin_dir, "rs-harness", &marker);
    support::write_activation(&root, &[support::provider("rust", Vec::new())]);

    let output = support::asp_command(&root)
        .env("PATH", support::prepend_path(&bin_dir))
        .env("PRJ_CACHE_HOME", root.join(".cache"))
        .args([
            "rust",
            "query",
            "--selector",
            "rust://src/lib.rs#item/function/missing_code_fixture",
            "--workspace",
            ".",
            "--code",
        ])
        .output()
        .expect("run asp rust query structural selector code miss");

    assert!(
        !output.status.success(),
        "missing structural selector must not exit successfully"
    );
    assert!(
        output.stdout.is_empty(),
        "missing structural selector must not emit empty-success evidence"
    );
    let stderr = String::from_utf8(output.stderr).expect("stderr");
    assert!(
        stderr.contains("exact selector matched no owner item"),
        "{stderr}"
    );
    assert!(stderr.contains("ownerPath=src/lib.rs"), "{stderr}");
    assert!(
        stderr.contains("itemQuery=missing_code_fixture"),
        "{stderr}"
    );
    assert!(stderr.contains("selectorKind=function"), "{stderr}");
    assert!(stderr.contains("state=not-found"), "{stderr}");
    assert!(stderr.contains("reason=item-not-found"), "{stderr}");
    assert!(
        stderr.contains("recommendedNext=asp rust search owner src/lib.rs items --query missing_code_fixture --workspace . --view seeds"),
        "{stderr}"
    );
    assert!(
        !marker.exists(),
        "missing structural selector code query should not spawn provider"
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn query_bare_file_selector_without_code_remains_owner_inventory() {
    let root = support::temp_project_root("query-bare-file-selector-inventory");
    let bin_dir = root.join(".bin");
    let marker = root.join("provider-called");
    std::fs::create_dir_all(root.join("src")).expect("create src");
    std::fs::write(root.join("src/lib.rs"), "pub fn direct_code_fixture() {}\n")
        .expect("write source");
    support::write_marker_provider(&bin_dir, "rs-harness", &marker);
    support::write_activation(&root, &[support::provider("rust", Vec::new())]);

    let output = support::asp_command(&root)
        .env("PATH", support::prepend_path(&bin_dir))
        .env("PRJ_CACHE_HOME", root.join(".cache"))
        .args([
            "rust",
            "query",
            "--selector",
            "src/lib.rs",
            "--term",
            "direct_code_fixture",
            "--workspace",
            ".",
        ])
        .output()
        .expect("run asp rust query selector inventory");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("stdout");
    assert!(stdout.starts_with("[search-owner]"), "{stdout}");
    assert!(stdout.contains("direct_code_fixture"), "{stdout}");
    assert!(
        !marker.exists(),
        "bare selector inventory query should not spawn provider"
    );
    let _ = std::fs::remove_dir_all(root);
}
