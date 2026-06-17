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
    let source_trace = payload["sourceTrace"].as_array().expect("sourceTrace");
    assert_eq!(source_trace[0]["source"], "finder");
    assert_eq!(source_trace[0]["status"], "used");
    assert_eq!(source_trace[0]["matched"], 2);
    assert_eq!(source_trace[0]["missing"], 0);
    assert_eq!(source_trace[0]["normalized"], 2);
    assert!(source_trace[0]["fields"]["elapsedMs"].is_number());
    assert!(
        source_trace
            .iter()
            .any(|trace| trace["source"].as_str() == Some("providerFacts")),
        "{payload}"
    );
    assert_eq!(
        payload["surfaces"],
        serde_json::json!(["owner", "items", "tests", "topology"])
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

#[test]
fn search_pipe_finder_respects_gitignore_and_configured_hidden_dirs() {
    let root = temp_project_root("search-pipe-finder-ignore-walk");
    let bin_dir = root.join(".bin");
    let marker = root.join("provider-called");
    for path in ["src", "ignored/src", ".allowed/src", ".blocked/src"] {
        std::fs::create_dir_all(root.join(path)).expect("create source dir");
    }
    std::fs::write(root.join(".gitignore"), "ignored/\n").expect("write gitignore");
    std::fs::write(
        root.join("asp.toml"),
        "[search]\nincludeHiddenDirs = [\".allowed\"]\n",
    )
    .expect("write asp config");
    std::fs::write(root.join("src/lib.rs"), "pub struct VisibleHit;\n").expect("write visible");
    std::fs::write(root.join(".allowed/src/lib.rs"), "pub struct HiddenHit;\n")
        .expect("write allowed hidden");
    std::fs::write(root.join("ignored/src/lib.rs"), "pub struct IgnoredHit;\n")
        .expect("write ignored");
    std::fs::write(root.join(".blocked/src/lib.rs"), "pub struct BlockedHit;\n")
        .expect("write blocked hidden");
    write_marker_provider(&bin_dir, "rs-harness", &marker);
    write_activation(&root, &[provider("rust", Vec::new())]);

    let output = asp_command(&root)
        .env("PATH", &bin_dir)
        .env("PRJ_CACHE_HOME", root.join(".cache"))
        .args([
            "rust",
            "search",
            "pipe",
            "VisibleHit HiddenHit IgnoredHit BlockedHit",
            "--source",
            "finder",
            "--view",
            "graph-turbo-request",
            ".",
        ])
        .output()
        .expect("run asp rust finder graph request");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let payload: Value = serde_json::from_slice(&output.stdout).expect("graph request json");
    assert_graph_turbo_request_contract(&payload);
    let payload_text = serde_json::to_string(&payload).expect("serialize payload");
    assert!(payload_text.contains("src/lib.rs"), "{payload_text}");
    assert!(
        payload_text.contains(".allowed/src/lib.rs"),
        "{payload_text}"
    );
    assert!(
        !payload_text.contains("ignored/src/lib.rs"),
        "{payload_text}"
    );
    assert!(
        !payload_text.contains(".blocked/src/lib.rs"),
        "{payload_text}"
    );
    assert!(!marker.exists(), "search pipe should not spawn provider");
    let _ = std::fs::remove_dir_all(root);
}
