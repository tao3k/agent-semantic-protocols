use crate::provider_command::facade::pipe::lexical::support::refresh_source_index;
use crate::provider_command::support;

use serde_json::Value;

#[test]
fn lexical_seeds_use_source_index_when_warm() {
    let root = support::temp_project_root("search-lexical-source-index");
    let bin_dir = root.join(".bin");
    let marker = root.join("provider-called");
    std::fs::create_dir_all(root.join("src")).expect("create src");
    std::fs::write(
        root.join("Cargo.toml"),
        "[package]\nname = \"search-lexical-source-index\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
    )
    .expect("write rust package anchor");
    std::fs::write(
        root.join("src/lib.rs"),
        "pub fn source_index_fixture() {}\npub fn unrelated() {}\n",
    )
    .expect("write source");
    support::write_marker_provider(&bin_dir, "rs-harness", &marker);
    support::write_activation(&root, &[support::provider("rust", Vec::new())]);
    refresh_source_index(&root);
    let _ = std::fs::remove_file(&marker);

    let output = support::asp_command(&root)
        .env("PATH", support::prepend_path(&bin_dir))
        .env("PRJ_CACHE_HOME", root.join(".cache"))
        .args([
            "rust",
            "search",
            "lexical",
            "source_index_fixture|unrelated",
            "owner",
            "items",
            "tests",
            "--workspace",
            ".",
            "--view",
            "seeds",
        ])
        .output()
        .expect("run asp rust search lexical");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("stdout");
    assert!(stdout.starts_with("[graph-route]"), "{stdout}");
    assert!(stdout.contains("owner=path(src/lib.rs)"), "{stdout}");
    assert!(stdout.contains("symbols=source_index_fixture"), "{stdout}");
    assert!(
        !marker.exists(),
        "search-overlay lexical path should not spawn provider"
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn lexical_frontier_receipt_out_is_asp_owned_runtime_capture() {
    let root = support::temp_project_root("search-lexical-frontier-receipt-out");
    let bin_dir = root.join(".bin");
    let marker = root.join("provider-called");
    let receipt_path = root.join("frontier-receipt.json");
    std::fs::create_dir_all(root.join("src")).expect("create src");
    std::fs::write(
        root.join("src/lib.rs"),
        "pub fn cache_root() {}\npub fn unrelated() {}\n",
    )
    .expect("write source");
    support::write_marker_provider(&bin_dir, "rs-harness", &marker);
    support::write_activation(&root, &[support::provider("rust", Vec::new())]);

    let output = support::asp_command(&root)
        .env("PATH", support::prepend_path(&bin_dir))
        .env("PRJ_CACHE_HOME", root.join(".cache"))
        .args([
            "rust",
            "search",
            "lexical",
            "cache_root|unrelated",
            "owner",
            "items",
            "tests",
            "--view",
            "seeds",
            "--frontier-receipt-out",
            receipt_path.to_str().expect("receipt path"),
            "--frontier-receipt-follow-node",
            "query:cache_root",
            "--frontier-receipt-read-selector",
            "src/lib.rs:1:1",
            "--frontier-receipt-read-kind",
            "direct-source-read",
            "--frontier-receipt-test-argv-json",
            "[\"cargo\",\"test\"]",
            "--frontier-receipt-test-status",
            "passed",
            "--frontier-receipt-test-summary",
            "1 passed",
            "--frontier-receipt-test-exit-code",
            "0",
            "--frontier-receipt-commands-to-first-useful-locator",
            "1",
            "--frontier-receipt-commands-to-validation",
            "2",
            ".",
        ])
        .output()
        .expect("run asp rust search lexical with frontier receipt");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("stdout");
    assert_builtin_graph_route(&stdout, "cache_root");
    let receipt: Value =
        serde_json::from_slice(&std::fs::read(&receipt_path).expect("read receipt"))
            .expect("receipt JSON");
    assert_eq!(
        receipt["schemaId"],
        "agent.semantic-protocols.semantic-fact-frontier-receipt"
    );
    assert!(
        !marker.exists(),
        "--frontier-receipt-out should not reach provider"
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn lexical_scoped_root_outputs_workspace_relative_replayable_locators() {
    let root = support::temp_project_root("search-lexical-scoped-root-locators");
    let bin_dir = root.join(".bin");
    let marker = root.join("provider-called");
    std::fs::create_dir_all(root.join("crates/demo/src")).expect("create scoped src");
    std::fs::write(
        root.join("Cargo.toml"),
        "[workspace]\nmembers = [\"crates/demo\"]\n",
    )
    .expect("write workspace manifest");
    std::fs::write(
        root.join("crates/demo/Cargo.toml"),
        "[package]\nname = \"demo\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
    )
    .expect("write demo manifest");
    std::fs::write(
        root.join("crates/demo/src/lib.rs"),
        "pub fn cache_root() {}\npub fn unrelated() {}\n",
    )
    .expect("write scoped source");
    support::write_marker_provider(&bin_dir, "rs-harness", &marker);
    support::write_activation(&root, &[support::provider("rust", Vec::new())]);

    let output = support::asp_command(&root)
        .env("PATH", support::prepend_path(&bin_dir))
        .env("PRJ_CACHE_HOME", root.join(".cache"))
        .args([
            "rust",
            "search",
            "lexical",
            "cache_root|unrelated",
            "owner",
            "items",
            "tests",
            "--view",
            "seeds",
            "crates/demo",
        ])
        .output()
        .expect("run scoped asp rust search lexical");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("stdout");
    assert!(
        stdout.contains("owner=path(crates/demo/src/lib.rs)"),
        "{stdout}"
    );
    assert!(stdout.contains("symbols=cache_root"), "{stdout}");
    assert!(
        stdout.contains("next=asp rust search owner 'crates/demo/src/lib.rs' items --query"),
        "{stdout}"
    );
    assert!(
        !stdout.contains("I=item:symbol(cache_root)@crates/demo/src/lib.rs:1:1"),
        "{stdout}"
    );

    assert!(
        !marker.exists(),
        "scoped fast path should not spawn provider"
    );
    let _ = std::fs::remove_dir_all(root);
}
use super::assert_builtin_graph_route;
