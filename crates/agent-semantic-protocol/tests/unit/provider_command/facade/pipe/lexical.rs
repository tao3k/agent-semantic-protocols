use crate::provider_command::support::{
    asp_command, prepend_path, provider, temp_project_root, write_activation, write_marker_provider,
};
use serde_json::Value;

use super::assert_graph_turbo_request_contract;

#[test]
fn lexical_rejects_owner_dependency_surface_combination_without_provider_spawn() {
    let root = temp_project_root("search-lexical-owner-deps-rejected");
    let bin_dir = root.join(".bin");
    let marker = root.join("provider-called");
    write_marker_provider(&bin_dir, "rs-harness", &marker);
    write_activation(&root, &[provider("rust", Vec::new())]);

    let output = asp_command(&root)
        .env("PATH", prepend_path(&bin_dir))
        .env("PRJ_CACHE_HOME", root.join(".cache"))
        .args([
            "rust",
            "search",
            "lexical",
            "gxpkg",
            "owner",
            "deps",
            "--workspace",
            ".",
            "--view",
            "seeds",
        ])
        .output()
        .expect("run asp rust search lexical owner deps");

    assert!(!output.status.success(), "search unexpectedly succeeded");
    let stderr = String::from_utf8(output.stderr).expect("stderr");
    assert!(
        stderr.contains("search lexical does not support combining deps with owner/items"),
        "{stderr}"
    );
    assert!(
        !marker.exists(),
        "rejected lexical surface combination should not spawn provider"
    );
    let _ = std::fs::remove_dir_all(root);
}

fn refresh_source_index(root: &std::path::Path) {
    let output = asp_command(root)
        .args(["cache", "source-index", "refresh"])
        .output()
        .expect("run asp cache source-index refresh");
    assert!(
        output.status.success(),
        "source-index refresh failed\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn lexical_seeds_is_asp_owned_for_cheap_discovery() {
    let root = temp_project_root("search-lexical-fast-facade");
    let bin_dir = root.join(".bin");
    let marker = root.join("provider-called");
    std::fs::create_dir_all(root.join("src")).expect("create src");
    std::fs::write(
        root.join("src/lib.rs"),
        "pub fn cache_root() {}\npub fn unrelated() {}\n",
    )
    .expect("write source");
    write_marker_provider(&bin_dir, "rs-harness", &marker);
    write_activation(&root, &[provider("rust", Vec::new())]);

    let output = asp_command(&root)
        .env("PATH", prepend_path(&bin_dir))
        .env("PRJ_CACHE_HOME", root.join(".cache"))
        .args([
            "rust",
            "search",
            "lexical",
            "cache_root",
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
    assert_builtin_graph_frontier(&stdout, "cache_root");
    assert!(
        !marker.exists(),
        "search lexical seeds should not spawn provider"
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn query_bare_file_selector_code_returns_source_not_owner_frontier() {
    let root = temp_project_root("query-bare-file-selector-code");
    let bin_dir = root.join(".bin");
    let marker = root.join("provider-called");
    std::fs::create_dir_all(root.join("src")).expect("create src");
    std::fs::write(
        root.join("src/lib.rs"),
        "pub fn direct_code_fixture() {}\npub fn unrelated() {}\n",
    )
    .expect("write source");
    write_marker_provider(&bin_dir, "rs-harness", &marker);
    write_activation(&root, &[provider("rust", Vec::new())]);

    let output = asp_command(&root)
        .env("PATH", prepend_path(&bin_dir))
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
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("stdout");
    assert_eq!(
        stdout,
        "pub fn direct_code_fixture() {}\npub fn unrelated() {}\n"
    );
    assert!(!stdout.contains("[search-owner]"), "{stdout}");
    assert!(
        !marker.exists(),
        "bare selector code query should not spawn provider"
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn query_structural_item_selector_code_returns_item_code() {
    let root = temp_project_root("query-structural-item-selector-code");
    let bin_dir = root.join(".bin");
    let marker = root.join("provider-called");
    std::fs::create_dir_all(root.join("src")).expect("create src");
    std::fs::write(
        root.join("src/lib.rs"),
        "pub fn direct_code_fixture() {\n    let value = 1;\n}\npub fn unrelated() {}\n",
    )
    .expect("write source");
    write_marker_provider(&bin_dir, "rs-harness", &marker);
    write_activation(&root, &[provider("rust", Vec::new())]);

    let output = asp_command(&root)
        .env("PATH", prepend_path(&bin_dir))
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
fn query_bare_file_selector_without_code_remains_owner_inventory() {
    let root = temp_project_root("query-bare-file-selector-inventory");
    let bin_dir = root.join(".bin");
    let marker = root.join("provider-called");
    std::fs::create_dir_all(root.join("src")).expect("create src");
    std::fs::write(root.join("src/lib.rs"), "pub fn direct_code_fixture() {}\n")
        .expect("write source");
    write_marker_provider(&bin_dir, "rs-harness", &marker);
    write_activation(&root, &[provider("rust", Vec::new())]);

    let output = asp_command(&root)
        .env("PATH", prepend_path(&bin_dir))
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

#[test]
fn lexical_seeds_use_source_index_when_warm() {
    let root = temp_project_root("search-lexical-source-index");
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
    write_marker_provider(&bin_dir, "rs-harness", &marker);
    write_activation(&root, &[provider("rust", Vec::new())]);
    refresh_source_index(&root);
    let _ = std::fs::remove_file(&marker);

    let output = asp_command(&root)
        .env("PATH", prepend_path(&bin_dir))
        .env("PRJ_CACHE_HOME", root.join(".cache"))
        .args([
            "rust",
            "search",
            "lexical",
            "source_index_fixture",
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
    assert!(stdout.starts_with("[search-lexical]"), "{stdout}");
    assert!(stdout.contains("source=source-index"), "{stdout}");
    assert!(stdout.contains("sourceTrace=sourceIndex:used"), "{stdout}");
    assert!(stdout.contains("search-overlay:skipped"), "{stdout}");
    assert!(stdout.contains("O=owner:path(src/lib.rs)"), "{stdout}");
    assert!(
        !marker.exists(),
        "search-overlay lexical path should not spawn provider"
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn lexical_frontier_receipt_out_is_asp_owned_runtime_capture() {
    let root = temp_project_root("search-lexical-frontier-receipt-out");
    let bin_dir = root.join(".bin");
    let marker = root.join("provider-called");
    let receipt_path = root.join("frontier-receipt.json");
    std::fs::create_dir_all(root.join("src")).expect("create src");
    std::fs::write(root.join("src/lib.rs"), "pub fn cache_root() {}\n").expect("write source");
    write_marker_provider(&bin_dir, "rs-harness", &marker);
    write_activation(&root, &[provider("rust", Vec::new())]);

    let output = asp_command(&root)
        .env("PATH", prepend_path(&bin_dir))
        .env("PRJ_CACHE_HOME", root.join(".cache"))
        .args([
            "rust",
            "search",
            "lexical",
            "cache_root",
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
    assert_builtin_graph_frontier(&stdout, "cache_root");
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
    let root = temp_project_root("search-lexical-scoped-root-locators");
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
        "pub fn cache_root() {}\n",
    )
    .expect("write scoped source");
    write_marker_provider(&bin_dir, "rs-harness", &marker);
    write_activation(&root, &[provider("rust", Vec::new())]);

    let output = asp_command(&root)
        .env("PATH", prepend_path(&bin_dir))
        .env("PRJ_CACHE_HOME", root.join(".cache"))
        .args([
            "rust",
            "search",
            "lexical",
            "cache_root",
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
        stdout.contains("O=owner:path(crates/demo/src/lib.rs)"),
        "{stdout}"
    );
    assert!(
        stdout.contains(
            "I=item:symbol(cache_root)@rust://crates/demo/src/lib.rs#item/symbol/cache_root"
        ),
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

#[test]
fn lexical_can_emit_graph_turbo_request_for_live_candidate_frontier() {
    let root = temp_project_root("search-lexical-graph-turbo-request");
    let bin_dir = root.join(".bin");
    let marker = root.join("provider-called");
    std::fs::create_dir_all(root.join("src")).expect("create src");
    std::fs::write(
        root.join("src/lib.rs"),
        "pub fn cache_root() {}\npub fn unrelated() {}\n",
    )
    .expect("write source");
    write_marker_provider(&bin_dir, "rs-harness", &marker);
    write_activation(&root, &[provider("rust", Vec::new())]);

    let output = asp_command(&root)
        .env("PATH", prepend_path(&bin_dir))
        .env("PRJ_CACHE_HOME", root.join(".cache"))
        .args([
            "rust",
            "search",
            "lexical",
            "cache_root",
            "owner",
            "items",
            "tests",
            "--view",
            "graph-turbo-request",
            ".",
        ])
        .output()
        .expect("run asp rust search lexical graph turbo request");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let payload: Value = serde_json::from_slice(&output.stdout).expect("graph turbo request JSON");
    assert_graph_turbo_request_contract(&payload);
    assert_eq!(payload["profile"], "owner-query");
    assert_eq!(payload["seedIds"][0], "query:cache_root");
    assert_eq!(payload["seedPlan"]["reason"], "query");
    assert_eq!(payload["seedPlan"]["queryPresent"], true);
    assert_eq!(payload["seedPlan"]["querySeedPresent"], true);
    assert_eq!(payload["seedPlan"]["fallbackOwnerSeedCount"], 0);
    assert!(
        payload["graph"]["nodes"]
            .as_array()
            .expect("nodes")
            .iter()
            .any(|node| node["kind"] == "item" && node["value"] == "cache_root")
    );
    assert!(
        !marker.exists(),
        "search lexical graph-turbo-request should not spawn provider"
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn typescript_lexical_can_emit_typed_hot_request_for_live_candidate_frontier() {
    let root = temp_project_root("typescript-search-lexical-graph-turbo-request");
    let bin_dir = root.join(".bin");
    let marker = root.join("provider-called");
    std::fs::create_dir_all(root.join("src")).expect("create src");
    std::fs::write(
        root.join("src/index.ts"),
        "export function cacheRoot() {}\nexport function unrelated() {}\n",
    )
    .expect("write source");
    write_marker_provider(&bin_dir, "ts-harness", &marker);
    write_activation(&root, &[provider("typescript", Vec::new())]);

    let output = asp_command(&root)
        .env("PATH", prepend_path(&bin_dir))
        .env("PRJ_CACHE_HOME", root.join(".cache"))
        .args([
            "typescript",
            "search",
            "lexical",
            "cacheRoot",
            "owner",
            "items",
            "tests",
            "--view",
            "graph-turbo-request",
            ".",
        ])
        .output()
        .expect("run asp typescript search lexical graph turbo request");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let payload: Value = serde_json::from_slice(&output.stdout).expect("graph turbo request JSON");
    assert_graph_turbo_request_contract(&payload);
    assert_eq!(payload["profile"], "owner-query");
    assert_eq!(payload["seedIds"][0], "query:cacheroot");
    assert_graph_has_hot_code_path(&payload, "cacheroot");
    assert!(
        !marker.exists(),
        "typescript graph-turbo-request should not spawn provider"
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn lexical_default_view_uses_builtin_ranker_for_live_candidate_frontier() {
    let root = temp_project_root("search-lexical-default-graph-turbo");
    let bin_dir = root.join(".bin");
    let marker = root.join("provider-called");
    std::fs::create_dir_all(root.join("src")).expect("create src");
    std::fs::write(
        root.join("src/lib.rs"),
        "pub fn cache_root() {}\npub fn unrelated() {}\n",
    )
    .expect("write source");
    write_marker_provider(&bin_dir, "rs-harness", &marker);
    write_activation(&root, &[provider("rust", Vec::new())]);

    let output = asp_command(&root)
        .env("PATH", prepend_path(&bin_dir))
        .env("PRJ_CACHE_HOME", root.join(".cache"))
        .args([
            "rust",
            "search",
            "lexical",
            "cache_root",
            "owner",
            "items",
            "tests",
            ".",
        ])
        .output()
        .expect("run asp rust search lexical default view");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("stdout");
    assert_builtin_graph_frontier(&stdout, "cache_root");
    assert!(
        !marker.exists(),
        "search lexical default view should not spawn provider"
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn typescript_lexical_default_view_uses_shared_graph_turbo_ranker() {
    let root = temp_project_root("typescript-search-lexical-default-graph-turbo");
    let bin_dir = root.join(".bin");
    let marker = root.join("provider-called");
    std::fs::create_dir_all(root.join("src")).expect("create src");
    std::fs::write(
        root.join("src/index.ts"),
        "export function cacheRoot() {}\nexport function unrelated() {}\n",
    )
    .expect("write source");
    write_marker_provider(&bin_dir, "ts-harness", &marker);
    write_activation(&root, &[provider("typescript", Vec::new())]);

    let output = asp_command(&root)
        .env("PATH", prepend_path(&bin_dir))
        .env("PRJ_CACHE_HOME", root.join(".cache"))
        .args([
            "typescript",
            "search",
            "lexical",
            "cacheRoot",
            "owner",
            "items",
            "tests",
            ".",
        ])
        .output()
        .expect("run asp typescript search lexical default view");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        String::from_utf8(output.stdout)
            .expect("stdout")
            .contains("I=item:symbol(cacheroot)")
    );
    assert!(
        !marker.exists(),
        "typescript search lexical default view should not spawn provider"
    );
    let _ = std::fs::remove_dir_all(root);
}

fn assert_builtin_graph_frontier(stdout: &str, symbol: &str) {
    assert!(
        stdout.starts_with("[graph-frontier] profile=owner-query"),
        "{stdout}"
    );
    assert!(
        stdout.contains(&format!("I=item:symbol({symbol})")),
        "{stdout}"
    );
    assert!(
        stdout.contains(&format!("H=hot:range({symbol})")),
        "{stdout}"
    );
    assert!(stdout.contains("rank="), "{stdout}");
    assert!(stdout.contains("frontier="), "{stdout}");
}

fn assert_graph_has_hot_code_path(payload: &Value, symbol: &str) {
    let nodes = payload["graph"]["nodes"].as_array().expect("nodes");
    let edges = payload["graph"]["edges"].as_array().expect("edges");
    let hot_id = nodes
        .iter()
        .find(|node| node["kind"] == "hot" && node["symbol"] == symbol)
        .and_then(|node| node["id"].as_str())
        .expect("hot node for symbol");
    let item_id = nodes
        .iter()
        .find(|node| node["kind"] == "item" && node["value"] == symbol)
        .and_then(|node| node["id"].as_str())
        .expect("item node for symbol");

    assert!(
        edges.iter().any(|edge| {
            edge["source"] == item_id && edge["target"] == hot_id && edge["relation"] == "contains"
        }),
        "expected item -> hot contains edge for {symbol}"
    );
}
