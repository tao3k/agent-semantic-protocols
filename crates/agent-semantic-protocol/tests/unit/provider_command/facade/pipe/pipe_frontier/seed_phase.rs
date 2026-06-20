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
    assert!(stdout.contains("A2=fd-query("), "{stdout}");
    assert!(
        stdout.contains("recommendedNext=A1.rg-query-set"),
        "{stdout}"
    );
    assert!(
        stdout.contains("nextCommand=asp rg -query") && stdout.contains(" --workspace ."),
        "{stdout}"
    );
    assert_evidence_edges_reference_visible_nodes(&stdout);
    assert!(!marker.exists(), "search pipe should not spawn provider");
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn search_pipe_graph_turbo_request_ranks_dense_owner_seed_before_weak_local_item() {
    let root = temp_project_root("search-pipe-graph-owner-ranking");
    let bin_dir = root.join(".bin");
    let marker = root.join("provider-called");
    std::fs::create_dir_all(root.join("src/project_runtime")).expect("create source dirs");
    std::fs::write(root.join("src/aaa_graph.rs"), "pub fn graph_marker() {}\n")
        .expect("write weak source file");
    std::fs::write(
        root.join("src/bbb_project.rs"),
        "pub fn project_marker() {}\n",
    )
    .expect("write weak source file");
    std::fs::write(
        root.join("src/ccc_runtime.rs"),
        "pub fn runtime_marker() {}\n",
    )
    .expect("write weak source file");
    std::fs::write(
        root.join("src/project_runtime/session_content.rs"),
        [
            "pub struct ProjectRuntimeSessionContentSourceAnchor;\n",
            "impl ProjectRuntimeSessionContentSourceAnchor {\n",
            "    pub fn project_runtime_session_content_source_anchor(&self) {}\n",
            "}\n",
            "pub fn runtime_session_content_source() {}\n",
            "pub fn session_content_anchor_source() {}\n",
        ]
        .join(""),
    )
    .expect("write dense source file");
    write_marker_provider(&bin_dir, "rs-harness", &marker);
    write_activation(&root, &[provider("rust", Vec::new())]);

    let output = asp_command(&root)
        .env("PATH", prepend_path(&bin_dir))
        .env("PRJ_CACHE_HOME", root.join(".cache"))
        .args([
            "rust",
            "search",
            "pipe",
            "graph project runtime session content source anchor",
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
    let owner_seed_ids = payload["seedIds"]
        .as_array()
        .expect("seedIds")
        .iter()
        .filter_map(Value::as_str)
        .filter(|seed_id| seed_id.starts_with("owner:"))
        .collect::<Vec<_>>();
    assert_eq!(
        payload["seedPlan"]["queryOwnerSeedCount"].as_u64(),
        Some(2),
        "{payload}"
    );
    assert!(
        owner_seed_ids
            .first()
            .is_some_and(|seed_id| seed_id.contains("src/project_runtime/session_content.rs")),
        "dense owner should outrank weak first-seen owner: {payload}"
    );
    assert!(
        owner_seed_ids
            .iter()
            .any(|seed_id| seed_id.contains("src/aaa_graph.rs")),
        "test must include the weak first-seen owner as a competing seed: {payload}"
    );
    assert!(!marker.exists(), "search pipe should not spawn provider");
    let _ = std::fs::remove_dir_all(root);
}

fn assert_evidence_edges_reference_visible_nodes(stdout: &str) {
    let node_aliases = stdout
        .lines()
        .find_map(|line| line.strip_prefix("evidenceNodes="))
        .into_iter()
        .flat_map(|nodes| nodes.split(';'))
        .filter_map(|node| node.split_once('=').map(|(alias, _)| alias.to_string()))
        .collect::<std::collections::HashSet<_>>();
    let Some(edges) = stdout
        .lines()
        .find_map(|line| line.strip_prefix("evidenceEdges="))
    else {
        return;
    };
    for edge in edges.split(';') {
        let (source, targets) = edge.split_once(">{").expect("edge line");
        assert!(
            node_aliases.contains(source),
            "edge source {source} must be in evidenceNodes: {stdout}"
        );
        for target in targets.trim_end_matches('}').split(',') {
            let (alias, _) = target.split_once(':').expect("edge target");
            assert!(
                node_aliases.contains(alias),
                "edge target {alias} must be in evidenceNodes: {stdout}"
            );
        }
    }
}

#[test]
fn search_pipe_graph_turbo_request_prefers_package_axis_cluster_over_cross_package_first_seen() {
    let root = temp_project_root("search-pipe-graph-package-ranking");
    let bin_dir = root.join(".bin");
    let marker = root.join("provider-called");
    std::fs::create_dir_all(root.join("crates/agent-semantic-client/src"))
        .expect("create client source dir");
    std::fs::create_dir_all(root.join("crates/agent-semantic-protocol/src/command"))
        .expect("create protocol source dir");
    std::fs::write(
        root.join("crates/agent-semantic-client/src/tools_cli.rs"),
        "pub fn graph_turbo_owner_candidate_ranking_package_local_evidence() {}\n",
    )
    .expect("write client owner");
    std::fs::write(
        root.join(
            "crates/agent-semantic-protocol/src/command/search_pipe_graph_turbo_owner_rank.rs",
        ),
        "pub fn graph_turbo_owner_candidate_ranking_package_local_evidence() {}\n",
    )
    .expect("write protocol owner rank");
    std::fs::write(
        root.join("crates/agent-semantic-protocol/src/command/search_pipe_graph_nodes.rs"),
        "pub fn topology_monorepo_submodule_owner_graph() {}\n",
    )
    .expect("write protocol topology owner");
    std::fs::write(
        root.join("crates/agent-semantic-protocol/src/command/search_pipe_graph_turbo.rs"),
        "pub fn graph_turbo_topology_package() {}\n",
    )
    .expect("write protocol graph turbo owner");
    write_marker_provider(&bin_dir, "rs-harness", &marker);
    write_activation(&root, &[provider("rust", Vec::new())]);

    let output = asp_command(&root)
        .env("PATH", prepend_path(&bin_dir))
        .env("PRJ_CACHE_HOME", root.join(".cache"))
        .args([
            "rust",
            "search",
            "pipe",
            "graph turbo owner candidate ranking package topology local evidence monorepo submodule",
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
    let owner_ids = payload["graph"]["nodes"]
        .as_array()
        .expect("nodes")
        .iter()
        .filter(|node| node["kind"].as_str() == Some("owner"))
        .filter_map(|node| node["id"].as_str())
        .collect::<Vec<_>>();
    assert!(
        owner_ids.first().is_some_and(|id| id.contains(
            "crates/agent-semantic-protocol/src/command/search_pipe_graph_turbo_owner_rank.rs"
        )),
        "protocol owner rank package should outrank first-seen client owner: {payload}"
    );
    assert!(
        owner_ids
            .iter()
            .position(|id| id.contains(
                "crates/agent-semantic-protocol/src/command/search_pipe_graph_turbo_owner_rank.rs"
            ))
            .expect("protocol owner rank")
            < owner_ids
                .iter()
                .position(|id| id.contains("crates/agent-semantic-client/src/tools_cli.rs"))
                .expect("client owner"),
        "{payload}"
    );
    assert!(!marker.exists(), "search pipe should not spawn provider");
    let _ = std::fs::remove_dir_all(root);
}
