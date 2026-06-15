use crate::provider_command::support::{
    asp_command, prepend_path, provider, temp_project_root, write_activation, write_marker_provider,
};
use serde_json::Value;

#[test]
fn search_pipe_graph_turbo_request_adds_owner_anchor_seeds_for_broad_query() {
    let root = temp_project_root("search-pipe-graph-seed-phase");
    let bin_dir = root.join(".bin");
    let marker = root.join("provider-called");
    std::fs::create_dir_all(root.join("src")).expect("create src");
    for index in 0..4 {
        std::fs::write(
            root.join("src").join(format!("seed_{index}.rs")),
            format!("fn cache_runtime_graph_package_parser_owner_{index}() {{}}\n"),
        )
        .expect("write source file");
    }
    write_marker_provider(&bin_dir, "rs-harness", &marker);
    write_activation(&root, &[provider("rust", Vec::new())]);

    let output = asp_command(&root)
        .env("PATH", prepend_path(&bin_dir))
        .env("PRJ_CACHE_HOME", root.join(".cache"))
        .args([
            "rust",
            "search",
            "pipe",
            "CacheRuntime GraphPackage cache runtime graph package parser owner",
            "--workspace",
            ".",
            "--view",
            "graph-turbo-request",
            ".",
        ])
        .output()
        .expect("run asp rust search pipe graph request");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let payload: Value = serde_json::from_slice(&output.stdout).expect("graph request json");
    super::assert_graph_turbo_request_contract(&payload);
    let seed_ids = payload["seedIds"].as_array().expect("seedIds");
    assert_eq!(
        payload["seedPlan"]["queryOwnerSeedCount"].as_u64(),
        Some(2),
        "{payload}"
    );
    assert_eq!(
        payload["seedPlan"]["selectedSeedCount"].as_u64(),
        Some(seed_ids.len() as u64),
        "{payload}"
    );
    assert!(
        seed_ids
            .iter()
            .any(|seed_id| seed_id.as_str().is_some_and(|id| id.starts_with("query:"))),
        "{payload}"
    );
    assert_eq!(
        seed_ids
            .iter()
            .filter(|seed_id| seed_id
                .as_str()
                .is_some_and(|id| id.starts_with("owner:src/seed_")))
            .count(),
        2,
        "{payload}"
    );
    assert_eq!(payload["seedPlan"]["reason"].as_str(), Some("query"));
    assert_eq!(payload["seedPlan"]["seedQuality"].as_str(), Some("review"));
    assert!(
        payload["seedPlan"]["riskFactors"]
            .as_array()
            .expect("riskFactors")
            .iter()
            .any(|risk| risk.as_str() == Some("flat-query")),
        "{payload}"
    );
    assert!(
        payload["seedPlan"]["recommendedActions"]
            .as_array()
            .expect("recommendedActions")
            .iter()
            .any(|action| action.as_str() == Some("split-query-pack")),
        "{payload}"
    );
    let nodes = payload["graph"]["nodes"].as_array().expect("nodes");
    for seed_id in seed_ids {
        let seed_id = seed_id.as_str().expect("seed id string");
        assert!(
            nodes
                .iter()
                .any(|node| node["id"].as_str() == Some(seed_id)),
            "seed id must reference a graph node: {seed_id} in {payload}"
        );
    }
    let seeds_output = asp_command(&root)
        .env("PATH", prepend_path(&bin_dir))
        .env("PRJ_CACHE_HOME", root.join(".cache"))
        .args([
            "rust",
            "search",
            "pipe",
            "cache runtime graph package parser owner",
            "--workspace",
            ".",
            "--view",
            "seeds",
            ".",
        ])
        .output()
        .expect("run asp rust search pipe seeds view");
    assert!(
        seeds_output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&seeds_output.stderr)
    );
    let stdout = String::from_utf8(seeds_output.stdout).expect("stdout");
    assert!(
        stdout.contains("seedPlanDetail=quality=review queryOwnerSeedCount=2 selectedSeedCount=3"),
        "{stdout}"
    );
    assert!(
        stdout.contains("riskFactors=flat-query,owner-drift"),
        "{stdout}"
    );
    assert!(
        stdout.contains("recommendedActions=split-query-pack,narrow-owner-scope"),
        "{stdout}"
    );
    assert!(stdout.contains("A1=rg-query-set("), "{stdout}");
    assert!(
        stdout.contains("recommendedNext=A1.rg-query-set"),
        "{stdout}"
    );
    assert!(stdout.contains("nextCommand=asp rg -query"), "{stdout}");
    assert!(!marker.exists(), "search pipe should not spawn provider");
    let _ = std::fs::remove_dir_all(root);
}
