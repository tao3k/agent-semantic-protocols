use crate::provider_command::support::{
    asp_command, prepend_path, provider, temp_project_root, write_activation, write_marker_provider,
};
use serde_json::Value;

use super::assert_graph_turbo_request_contract;

#[test]
fn rust_lexical_query_deps_request_includes_import_dependency_facts() {
    let root = temp_project_root("rust-search-lexical-query-deps-graph-turbo-request");
    let bin_dir = root.join(".bin");
    let marker = root.join("provider-called");
    std::fs::create_dir_all(root.join("src")).expect("create src");
    std::fs::write(
        root.join("src/lib.rs"),
        "use serde_json::Value;\npub fn cache_root(value: Value) { let _ = value; }\n",
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
            "serde_json",
            "deps",
            "--view",
            "graph-turbo-request",
            ".",
        ])
        .output()
        .expect("run asp rust search lexical query deps");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let payload: Value = serde_json::from_slice(&output.stdout).expect("graph turbo request JSON");
    assert_graph_turbo_request_contract(&payload);
    assert_eq!(payload["profile"], "query-deps");
    assert_graph_has_dependency_import(&payload, "serde_json");
    assert!(
        !marker.exists(),
        "rust query-deps should not spawn provider"
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn typescript_lexical_query_deps_request_includes_import_dependency_facts() {
    let root = temp_project_root("typescript-search-lexical-query-deps-graph-turbo-request");
    let bin_dir = root.join(".bin");
    let marker = root.join("provider-called");
    std::fs::create_dir_all(root.join("src")).expect("create src");
    std::fs::write(
        root.join("src/index.ts"),
        "import { readFile } from 'node:fs';\nexport function cacheRoot() { return readFile; }\n",
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
            "node:fs",
            "deps",
            "--view",
            "graph-turbo-request",
            ".",
        ])
        .output()
        .expect("run asp typescript search lexical query deps");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let payload: Value = serde_json::from_slice(&output.stdout).expect("graph turbo request JSON");
    assert_graph_turbo_request_contract(&payload);
    assert_eq!(payload["profile"], "query-deps");
    assert_graph_has_dependency_import(&payload, "node:fs");
    assert!(
        !marker.exists(),
        "typescript query-deps should not spawn provider"
    );
    let _ = std::fs::remove_dir_all(root);
}

fn assert_graph_has_dependency_import(payload: &Value, dependency: &str) {
    let nodes = payload["graph"]["nodes"].as_array().expect("nodes");
    let edges = payload["graph"]["edges"].as_array().expect("edges");
    let dependency_id = nodes
        .iter()
        .find(|node| node["kind"] == "dependency" && node["value"] == dependency)
        .and_then(|node| node["id"].as_str())
        .expect("dependency node");
    let owner_ids = nodes
        .iter()
        .filter(|node| node["kind"] == "owner")
        .filter_map(|node| node["id"].as_str())
        .collect::<Vec<_>>();
    let query_id = nodes
        .iter()
        .find(|node| node["kind"] == "query")
        .and_then(|node| node["id"].as_str())
        .expect("query node");

    assert!(
        owner_ids.iter().any(|owner_id| {
            edges.iter().any(|edge| {
                edge["source"] == *owner_id
                    && edge["target"] == dependency_id
                    && edge["relation"] == "imports"
            })
        }),
        "expected owner -> dependency imports edge for {dependency}"
    );
    assert!(
        edges.iter().any(|edge| {
            edge["source"] == query_id
                && edge["target"] == dependency_id
                && edge["relation"] == "matches"
        }),
        "expected query -> dependency match edge for {dependency}"
    );
}
