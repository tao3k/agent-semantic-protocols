use crate::provider_command::support::{
    asp_command, prepend_path, provider, temp_project_root, write_activation, write_marker_provider,
};

use super::assert_graph_turbo_request_contract;
use serde_json::Value;

#[test]
fn search_pipe_surface_option_controls_graph_request_surfaces() {
    let root = temp_project_root("search-pipe-surface-option");
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
            "--surface",
            "owner,tests",
            "--view",
            "graph-turbo-request",
            ".",
        ])
        .output()
        .expect("run asp rust search pipe graph request with surfaces");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let payload: Value = serde_json::from_slice(&output.stdout).expect("graph request json");
    assert_graph_turbo_request_contract(&payload);
    assert_eq!(payload["surfaces"], serde_json::json!(["owner", "tests"]));
    assert_eq!(payload["profile"], "owner-tests");
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
            .any(|node| node["kind"].as_str() == Some("test")),
        "{payload}"
    );
    assert!(
        !nodes
            .iter()
            .any(|node| matches!(node["kind"].as_str(), Some("item" | "hot"))),
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
        !edges
            .iter()
            .any(|edge| edge["relation"].as_str() == Some("contains")),
        "{payload}"
    );
    assert!(!marker.exists(), "search pipe should not spawn provider");
    let _ = std::fs::remove_dir_all(root);
}
