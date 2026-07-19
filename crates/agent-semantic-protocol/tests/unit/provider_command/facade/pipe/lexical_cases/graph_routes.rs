use crate::provider_command::support;
use crate::unit::provider_command::facade::pipe::assert_graph_turbo_request_contract;
use serde_json::Value;

#[test]
fn lexical_can_emit_graph_turbo_request_for_live_candidate_frontier() {
    let root = support::temp_project_root("search-lexical-graph-turbo-request");
    let bin_dir = root.join(".bin");
    let marker = root.join("provider-called");
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
    let root = support::temp_project_root("typescript-search-lexical-graph-turbo-request");
    let bin_dir = root.join(".bin");
    let marker = root.join("provider-called");
    std::fs::create_dir_all(root.join("src")).expect("create src");
    std::fs::write(
        root.join("src/index.ts"),
        "export function cacheRoot() {}\nexport function unrelated() {}\n",
    )
    .expect("write source");
    support::write_marker_provider(&bin_dir, "ts-harness", &marker);
    support::write_activation(&root, &[support::provider("typescript", Vec::new())]);

    let output = support::asp_command(&root)
        .env("PATH", support::prepend_path(&bin_dir))
        .env("PRJ_CACHE_HOME", root.join(".cache"))
        .args([
            "typescript",
            "search",
            "lexical",
            "cacheRoot|unrelated",
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
    let root = support::temp_project_root("search-lexical-default-graph-turbo");
    let bin_dir = root.join(".bin");
    let marker = root.join("provider-called");
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
    assert_builtin_graph_route(&stdout, "cache_root");
    assert!(
        !marker.exists(),
        "search lexical default view should not spawn provider"
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn typescript_lexical_default_view_uses_shared_graph_turbo_ranker() {
    let root = support::temp_project_root("typescript-search-lexical-default-graph-turbo");
    let bin_dir = root.join(".bin");
    let marker = root.join("provider-called");
    std::fs::create_dir_all(root.join("src")).expect("create src");
    std::fs::write(
        root.join("src/index.ts"),
        "export function cacheRoot() {}\nexport function unrelated() {}\n",
    )
    .expect("write source");
    support::write_marker_provider(&bin_dir, "ts-harness", &marker);
    support::write_activation(&root, &[support::provider("typescript", Vec::new())]);

    let output = support::asp_command(&root)
        .env("PATH", support::prepend_path(&bin_dir))
        .env("PRJ_CACHE_HOME", root.join(".cache"))
        .args([
            "typescript",
            "search",
            "lexical",
            "cacheRoot|unrelated",
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
    let stdout = String::from_utf8(output.stdout).expect("stdout");
    assert!(stdout.starts_with("[graph-route]"), "{stdout}");
    assert!(stdout.contains("owner=path(src/index.ts)"), "{stdout}");
    assert!(
        stdout.contains("next=asp typescript search owner 'src/index.ts' items --query"),
        "{stdout}"
    );
    assert!(
        !marker.exists(),
        "typescript search lexical default view should not spawn provider"
    );
    let _ = std::fs::remove_dir_all(root);
}

fn assert_builtin_graph_route(stdout: &str, symbol: &str) {
    assert!(
        stdout.starts_with("[graph-route] profile=owner-query"),
        "{stdout}"
    );
    assert!(stdout.contains("relation=cohesive"), "{stdout}");
    assert!(stdout.contains("route=owner-item"), "{stdout}");
    assert!(stdout.contains("owner=path(src/lib.rs)"), "{stdout}");
    assert!(stdout.contains(&format!("symbols={symbol}")), "{stdout}");
    assert!(
        stdout.contains("next=asp rust search owner 'src/lib.rs' items --query"),
        "{stdout}"
    );
    assert!(!stdout.contains("rank="), "{stdout}");
    assert!(!stdout.contains("frontier="), "{stdout}");
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
