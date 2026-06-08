use crate::provider_command::support::{
    asp_command, prepend_path, provider, temp_project_root, write_activation, write_marker_provider,
};

use super::assert_graph_turbo_request_contract;
use serde_json::Value;

#[test]
fn search_pipe_source_option_controls_graph_request_source() {
    let root = temp_project_root("search-pipe-source-option");
    let bin_dir = root.join(".bin");
    let marker = root.join("provider-called");
    std::fs::create_dir_all(root.join("src")).expect("create src");
    std::fs::write(
        root.join("src/lib.rs"),
        "pub struct HookDecision;\npub struct ClientReceipt;\n",
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
            "pipe",
            "HookDecision ClientReceipt",
            "--source",
            "finder",
            "--view",
            "graph-turbo-request",
            ".",
        ])
        .output()
        .expect("run asp rust search pipe graph request with source");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let payload: Value = serde_json::from_slice(&output.stdout).expect("graph request json");
    assert_graph_turbo_request_contract(&payload);
    assert_eq!(payload["surface"], "search-pipe");
    assert_eq!(
        payload["queryTerms"],
        serde_json::json!(["HookDecision", "ClientReceipt"])
    );
    assert_eq!(payload["source"], "finder");
    assert_eq!(payload["candidateSources"], serde_json::json!(["finder"]));
    assert_eq!(
        payload["sourceTrace"],
        serde_json::json!([
            {
                "source": "finder",
                "status": "used",
                "matched": 2,
                "missing": 0,
                "normalized": 2
            }
        ])
    );
    assert_eq!(
        payload["surfaces"],
        serde_json::json!(["owner", "items", "tests"])
    );
    assert_eq!(payload["profile"], "owner-query");
    let nodes = payload["graph"]["nodes"].as_array().expect("nodes");
    assert!(
        nodes
            .iter()
            .any(|node| node["kind"].as_str() == Some("owner")),
        "{payload}"
    );
    assert!(
        nodes
            .iter()
            .any(|node| node["kind"].as_str() == Some("item")
                && node["source"].as_str() == Some("finder")
                && node["confidence"].as_str() == Some("heuristic")),
        "{payload}"
    );
    let edges = payload["graph"]["edges"].as_array().expect("edges");
    assert!(
        edges
            .iter()
            .any(|edge| edge["relation"].as_str() == Some("covers")),
        "{payload}"
    );
    assert!(
        edges
            .iter()
            .any(|edge| edge["relation"].as_str() == Some("contains")),
        "{payload}"
    );
    assert!(!marker.exists(), "search pipe should not spawn provider");
    let _ = std::fs::remove_dir_all(root);
}
