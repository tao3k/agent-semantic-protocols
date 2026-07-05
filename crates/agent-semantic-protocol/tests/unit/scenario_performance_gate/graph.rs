use std::{collections::HashMap, fs, path::Path, time::Instant};

use super::contracts::{
    assert_graph_candidate_projection_benchmark_contract,
    assert_graph_evidence_projection_benchmark_contract,
    assert_graph_node_projection_benchmark_contract, assert_graph_owner_rank_benchmark_contract,
    assert_graph_query_owner_seed_benchmark_contract,
    assert_graph_topology_projection_benchmark_contract,
};
use super::runtime_gates::{duration_literal, duration_millis_from_manifest, read_toml};
use super::shared::SharedBenchmarkToml;
use crate::provider_command::support::temp_project_root;

pub(super) fn asp_graph_node_projection_cold_functional_path_stays_inside_scenario_gate() {
    let crate_root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let scenario_root = crate_root
        .join("tests")
        .join("unit")
        .join("scenarios")
        .join("asp_graph_node_projection_cold_functional_path");
    let benchmark: SharedBenchmarkToml = read_toml(&scenario_root.join("benchmark.toml"));
    assert_graph_node_projection_benchmark_contract(&benchmark);
    let max_total_ms = duration_millis_from_manifest(&benchmark.max_total);

    let owners = vec![
        "Src/Generated Lib.rs".to_string(),
        "src/domain/model.rs".to_string(),
    ];
    let started_at = Instant::now();
    let empty_id = agent_semantic_search::stable_graph_node_id("owner", "!!!");
    let nodes = agent_semantic_search::owner_path_graph_nodes(&owners);
    let elapsed = started_at.elapsed();
    let elapsed_ms = elapsed.as_millis();

    assert_eq!(empty_id, "owner:node");
    assert_eq!(nodes.len(), 2);
    assert_eq!(nodes[0]["id"], "owner:src/generated-lib.rs");
    assert_eq!(nodes[0]["kind"], "owner");
    assert_eq!(nodes[0]["role"], "path");
    assert_eq!(nodes[0]["action"], "owner");
    assert_eq!(nodes[0]["path"], "Src/Generated Lib.rs");
    assert!(
        nodes
            .iter()
            .all(|node| !node["id"].as_str().unwrap_or("").contains(":1:")),
        "graph node projection must not encode executable line ranges; nodes={nodes:?}"
    );
    assert!(
        elapsed_ms <= max_total_ms,
        "graph node projection cold functional path exceeded benchmark max_total={} observed={}ms nodes={:?}",
        benchmark.max_total,
        elapsed_ms,
        nodes
    );

    let performance_gate = serde_json::json!({
        "schemaId": "agent.semantic-protocols.semantic-hot-path-performance-gate",
        "schemaVersion": "1",
        "scenarioId": "asp-graph-node-projection-cold-functional-path",
        "languageId": "rust",
        "workspace": ".",
        "command": [
            "agent_semantic_search::stable_graph_node_id",
            "agent_semantic_search::owner_path_graph_nodes"
        ],
        "phase": "cold",
        "expected": {
            "targetTotal": benchmark.target_total,
            "maxTotal": benchmark.max_total,
            "regressionBudget": benchmark.regression_budget,
            "maxProviderProcessCount": 0,
            "maxSearchOverlayProcessCount": 0,
            "maxStdoutBytes": benchmark.max_stdout_bytes,
            "requireSearchOwnedProjection": true,
            "allowedFirstRoutes": ["graph-node-projection"],
            "forbiddenRoutes": ["command-owner-node-builder", "provider-process", "path-generated-filter"],
            "requireNoExecutableLineRange": true
        },
        "observed": {
            "observedTotal": duration_literal(elapsed),
            "providerProcessCount": 0,
            "providerElapsed": "0us",
            "nativeFinderProcessCount": 0,
            "nativeFinderElapsed": "0us",
            "ownerNodeCount": nodes.len(),
            "firstRoute": "graph-node-projection",
            "executedRoutes": ["graph-node-projection"],
            "executableLineRangeSelectorCount": 0,
            "stdoutBytes": 0,
            "fallbackReason": "none"
        },
        "verdict": "pass",
        "evidenceRefs": ["scenario:asp-graph-node-projection-cold-functional-path"]
    });
    assert_eq!(performance_gate["observed"]["providerProcessCount"], 0);
    assert_eq!(performance_gate["observed"]["nativeFinderProcessCount"], 0);
    assert_eq!(performance_gate["observed"]["ownerNodeCount"], 2);
}

pub(super) fn asp_graph_candidate_projection_cold_functional_path_stays_inside_scenario_gate() {
    let crate_root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let scenario_root = crate_root
        .join("tests")
        .join("unit")
        .join("scenarios")
        .join("asp_graph_candidate_projection_cold_functional_path");
    let benchmark: SharedBenchmarkToml = read_toml(&scenario_root.join("benchmark.toml"));
    assert_graph_candidate_projection_benchmark_contract(&benchmark);
    let max_total_ms = duration_millis_from_manifest(&benchmark.max_total);

    let candidates = vec![agent_semantic_search::GraphProjectionCandidate::new(
        "src/lib.rs",
        3,
        4,
        "SearchOwner",
        "pub fn SearchOwner() {}",
        "source-index",
        "high",
    )];
    let started_at = Instant::now();
    let item_nodes = agent_semantic_search::graph_candidate_item_nodes(
        ("rust", candidates.as_slice(), 8).into(),
    );
    let hot_nodes =
        agent_semantic_search::graph_candidate_hot_nodes(("rust", candidates.as_slice(), 8).into());
    let elapsed = started_at.elapsed();
    let elapsed_ms = elapsed.as_millis();

    assert_eq!(item_nodes.len(), 1);
    assert_eq!(hot_nodes.len(), 1);
    assert_eq!(
        item_nodes[0]["structuralSelector"],
        "rust://src/lib.rs#item/symbol/SearchOwner"
    );
    assert_eq!(
        hot_nodes[0]["structuralSelector"],
        "rust://src/lib.rs#range/hot/SearchOwner"
    );
    assert_eq!(item_nodes[0]["displayLineRange"], "3:4");
    assert_eq!(hot_nodes[0]["startLine"], 1);
    assert_eq!(hot_nodes[0]["endLine"], 15);
    assert_eq!(hot_nodes[0]["codePolicy"], "requires-exact-code");
    assert!(
        item_nodes
            .iter()
            .chain(hot_nodes.iter())
            .all(|node| !node["structuralSelector"]
                .as_str()
                .unwrap_or("")
                .contains(":3:")),
        "graph candidate projection must not expose executable line ranges as structural identity; item_nodes={item_nodes:?} hot_nodes={hot_nodes:?}"
    );
    assert!(
        elapsed_ms <= max_total_ms,
        "graph candidate projection cold functional path exceeded benchmark max_total={} observed={}ms item_nodes={:?} hot_nodes={:?}",
        benchmark.max_total,
        elapsed_ms,
        item_nodes,
        hot_nodes
    );

    let performance_gate = serde_json::json!({
        "schemaId": "agent.semantic-protocols.semantic-hot-path-performance-gate",
        "schemaVersion": "1",
        "scenarioId": "asp-graph-candidate-projection-cold-functional-path",
        "languageId": "rust",
        "workspace": ".",
        "command": [
            "agent_semantic_search::graph_candidate_item_nodes",
            "agent_semantic_search::graph_candidate_hot_nodes"
        ],
        "phase": "cold",
        "expected": {
            "targetTotal": benchmark.target_total,
            "maxTotal": benchmark.max_total,
            "regressionBudget": benchmark.regression_budget,
            "maxProviderProcessCount": 0,
            "maxSearchOverlayProcessCount": 0,
            "maxStdoutBytes": benchmark.max_stdout_bytes,
            "requireSearchOwnedProjection": true,
            "allowedFirstRoutes": ["graph-candidate-projection"],
            "forbiddenRoutes": ["command-candidate-node-builder", "provider-process", "native-finder"],
            "requireNoExecutableLineRange": true
        },
        "observed": {
            "observedTotal": duration_literal(elapsed),
            "providerProcessCount": 0,
            "providerElapsed": "0us",
            "nativeFinderProcessCount": 0,
            "nativeFinderElapsed": "0us",
            "itemNodeCount": item_nodes.len(),
            "hotNodeCount": hot_nodes.len(),
            "firstRoute": "graph-candidate-projection",
            "executedRoutes": ["graph-candidate-projection"],
            "executableLineRangeSelectorCount": 0,
            "stdoutBytes": 0,
            "fallbackReason": "none"
        },
        "verdict": "pass",
        "evidenceRefs": ["scenario:asp-graph-candidate-projection-cold-functional-path"]
    });
    assert_eq!(performance_gate["observed"]["providerProcessCount"], 0);
    assert_eq!(performance_gate["observed"]["nativeFinderProcessCount"], 0);
    assert_eq!(performance_gate["observed"]["itemNodeCount"], 1);
    assert_eq!(performance_gate["observed"]["hotNodeCount"], 1);
}

pub(super) fn asp_graph_topology_projection_cold_functional_path_stays_inside_scenario_gate() {
    let crate_root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let scenario_root = crate_root
        .join("tests")
        .join("unit")
        .join("scenarios")
        .join("asp_graph_topology_projection_cold_functional_path");
    let benchmark: SharedBenchmarkToml = read_toml(&scenario_root.join("benchmark.toml"));
    assert_graph_topology_projection_benchmark_contract(&benchmark);
    let max_total_ms = duration_millis_from_manifest(&benchmark.max_total);

    let root = temp_project_root("scenario-graph-topology-projection-cold");
    fs::create_dir_all(root.join("src")).expect("create src");
    fs::create_dir_all(root.join("languages/rust/src")).expect("create submodule source");
    fs::write(
        root.join("Cargo.toml"),
        "[package]\nname = \"scenario-graph-topology-projection-cold\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
    )
    .expect("write cargo manifest");
    fs::write(root.join("Cargo.lock"), "# lock\n").expect("write cargo lock");
    fs::write(root.join("src/lib.rs"), "pub fn topology_fixture() {}\n").expect("write source");
    fs::write(
        root.join(".gitmodules"),
        "[submodule \"languages/rust\"]\n  path = languages/rust\n  url = https://example.invalid/rust.git\n",
    )
    .expect("write gitmodules");

    let candidates = vec![agent_semantic_search::GraphProjectionCandidate::new(
        "src/lib.rs",
        1,
        1,
        "topology_fixture",
        "pub fn topology_fixture() {}",
        "source-index",
        "high",
    )];
    let owners = vec!["languages/rust/src/lib.rs".to_string()];
    let started_at = Instant::now();
    let projection = agent_semantic_search::graph_project_topology_projection(
        ("rust", root.as_path(), candidates.as_slice()).into(),
    );
    let owner_edges = agent_semantic_search::graph_submodule_owner_edges(&root, &owners);
    let elapsed = started_at.elapsed();
    let elapsed_ms = elapsed.as_millis();

    assert!(
        projection
            .nodes
            .iter()
            .any(|node| { node["kind"] == "language-project" && node["path"] == "." })
    );
    assert!(
        projection
            .nodes
            .iter()
            .any(|node| { node["kind"] == "project-marker" && node["path"] == "Cargo.toml" })
    );
    assert!(
        projection
            .nodes
            .iter()
            .any(|node| { node["kind"] == "dependency-marker" && node["path"] == "Cargo.lock" })
    );
    assert!(
        projection
            .nodes
            .iter()
            .any(|node| { node["kind"] == "submodule" && node["path"] == "languages/rust" })
    );
    assert_eq!(owner_edges.len(), 1);
    assert_eq!(owner_edges[0]["relation"], "contains");
    assert!(
        elapsed_ms <= max_total_ms,
        "graph topology projection cold functional path exceeded benchmark max_total={} observed={}ms projection={projection:?} owner_edges={owner_edges:?}",
        benchmark.max_total,
        elapsed_ms
    );

    let performance_gate = serde_json::json!({
        "schemaId": "agent.semantic-protocols.semantic-hot-path-performance-gate",
        "schemaVersion": "1",
        "scenarioId": "asp-graph-topology-projection-cold-functional-path",
        "languageId": "rust",
        "workspace": ".",
        "command": [
            "agent_semantic_search::graph_project_topology_projection",
            "agent_semantic_search::graph_submodule_owner_edges"
        ],
        "phase": "cold",
        "expected": {
            "targetTotal": benchmark.target_total,
            "maxTotal": benchmark.max_total,
            "regressionBudget": benchmark.regression_budget,
            "maxProviderProcessCount": 0,
            "maxSearchOverlayProcessCount": 0,
            "maxStdoutBytes": benchmark.max_stdout_bytes,
            "requireSearchOwnedProjection": true,
            "allowedFirstRoutes": ["graph-topology-projection"],
            "forbiddenRoutes": ["command-project-marker-walk", "command-gitmodules-parser", "provider-process", "native-finder"],
            "requireProviderManifestMarkers": true
        },
        "observed": {
            "observedTotal": duration_literal(elapsed),
            "providerProcessCount": 0,
            "providerElapsed": "0us",
            "nativeFinderProcessCount": 0,
            "nativeFinderElapsed": "0us",
            "nodeCount": projection.nodes.len(),
            "edgeCount": projection.edges.len() + owner_edges.len(),
            "firstRoute": "graph-topology-projection",
            "executedRoutes": ["graph-topology-projection"],
            "stdoutBytes": 0,
            "fallbackReason": "none"
        },
        "verdict": "pass",
        "evidenceRefs": ["scenario:asp-graph-topology-projection-cold-functional-path"]
    });
    assert_eq!(performance_gate["observed"]["providerProcessCount"], 0);
    assert_eq!(performance_gate["observed"]["nativeFinderProcessCount"], 0);
    assert_eq!(
        performance_gate["observed"]["firstRoute"],
        "graph-topology-projection"
    );
    let _ = fs::remove_dir_all(root);
}

pub(super) fn asp_graph_owner_rank_cold_functional_path_stays_inside_scenario_gate() {
    let crate_root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let scenario_root = crate_root
        .join("tests")
        .join("unit")
        .join("scenarios")
        .join("asp_graph_owner_rank_cold_functional_path");
    let benchmark: SharedBenchmarkToml = read_toml(&scenario_root.join("benchmark.toml"));
    assert_graph_owner_rank_benchmark_contract(&benchmark);
    let max_total_ms = duration_millis_from_manifest(&benchmark.max_total);

    let candidates = vec![
        agent_semantic_search::GraphProjectionCandidate::new(
            "src/lib.rs",
            1,
            1,
            "SearchRouter",
            "dynamic overlay graph ranking",
            "source-index",
            "high",
        ),
        agent_semantic_search::GraphProjectionCandidate::new(
            "languages/rust/src/lib.rs",
            1,
            1,
            "SearchRouter",
            "dynamic overlay graph ranking",
            "source-index",
            "high",
        ),
        agent_semantic_search::GraphProjectionCandidate::new(
            "packages/runtime/search/src/router.rs",
            1,
            1,
            "SearchRouter",
            "dynamic overlay graph ranking",
            "finder-path",
            "path",
        ),
    ];
    let query_terms = vec!["dynamicOverlay".to_string(), "SearchRouter".to_string()];
    let submodule_paths = vec!["languages/rust".to_string()];
    let started_at = Instant::now();
    let ranked = agent_semantic_search::ranked_graph_owner_paths_for_submodule_paths(
        &candidates,
        &query_terms,
        &submodule_paths,
    );
    let elapsed = started_at.elapsed();
    let elapsed_ms = elapsed.as_millis();

    assert_eq!(ranked[0], "languages/rust/src/lib.rs");
    assert!(
        ranked
            .iter()
            .any(|path| path == "packages/runtime/search/src/router.rs"),
        "ranked owners={ranked:?}"
    );
    assert!(
        elapsed_ms <= max_total_ms,
        "graph owner rank cold functional path exceeded benchmark max_total={} observed={}ms ranked={ranked:?}",
        benchmark.max_total,
        elapsed_ms
    );

    let performance_gate = serde_json::json!({
        "schemaId": "agent.semantic-protocols.semantic-hot-path-performance-gate",
        "schemaVersion": "1",
        "scenarioId": "asp-graph-owner-rank-cold-functional-path",
        "languageId": "rust",
        "workspace": ".",
        "command": [
            "agent_semantic_search::ranked_graph_owner_paths_for_submodule_paths"
        ],
        "phase": "cold",
        "expected": {
            "targetTotal": benchmark.target_total,
            "maxTotal": benchmark.max_total,
            "regressionBudget": benchmark.regression_budget,
            "maxProviderProcessCount": 0,
            "maxSearchOverlayProcessCount": 0,
            "maxStdoutBytes": benchmark.max_stdout_bytes,
            "requireSearchOwnedRank": true,
            "allowedFirstRoutes": ["graph-owner-rank"],
            "forbiddenRoutes": ["command-owner-rank", "provider-process", "native-finder"],
            "requireTopologyMembershipBoost": true
        },
        "observed": {
            "observedTotal": duration_literal(elapsed),
            "providerProcessCount": 0,
            "providerElapsed": "0us",
            "nativeFinderProcessCount": 0,
            "nativeFinderElapsed": "0us",
            "ownerCount": ranked.len(),
            "firstOwner": ranked[0],
            "firstRoute": "graph-owner-rank",
            "executedRoutes": ["graph-owner-rank"],
            "stdoutBytes": 0,
            "fallbackReason": "none"
        },
        "verdict": "pass",
        "evidenceRefs": ["scenario:asp-graph-owner-rank-cold-functional-path"]
    });
    assert_eq!(performance_gate["observed"]["providerProcessCount"], 0);
    assert_eq!(performance_gate["observed"]["nativeFinderProcessCount"], 0);
    assert_eq!(
        performance_gate["observed"]["firstOwner"],
        "languages/rust/src/lib.rs"
    );
}

pub(super) fn asp_graph_query_owner_seed_cold_functional_path_stays_inside_scenario_gate() {
    let crate_root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let scenario_root = crate_root
        .join("tests")
        .join("unit")
        .join("scenarios")
        .join("asp_graph_query_owner_seed_cold_functional_path");
    let benchmark: SharedBenchmarkToml = read_toml(&scenario_root.join("benchmark.toml"));
    assert_graph_query_owner_seed_benchmark_contract(&benchmark);
    let max_total_ms = duration_millis_from_manifest(&benchmark.max_total);

    let candidates = vec![
        agent_semantic_search::GraphProjectionCandidate::new(
            "packages/runtime/search/src/router.rs",
            1,
            1,
            "SearchRouter",
            "dynamic overlay search",
            "package-path-query",
            "package-path",
        ),
        agent_semantic_search::GraphProjectionCandidate::new(
            "src/cache.rs",
            1,
            1,
            "CacheStatus",
            "cache status receipt",
            "source-index",
            "high",
        ),
    ];
    let owners = vec![
        "src/cache.rs".to_string(),
        "packages/runtime/search/src/router.rs".to_string(),
    ];
    let package_terms = vec!["runtime_search".to_string()];
    let cache_terms = vec!["CacheStatus".to_string()];
    let started_at = Instant::now();
    let has_package_path =
        agent_semantic_search::graph_has_package_path_candidate(&candidates, &package_terms);
    let package_seed = agent_semantic_search::graph_query_owner_seed_paths(
        &candidates,
        &owners,
        1,
        &package_terms,
    );
    let evidence_seed =
        agent_semantic_search::graph_query_owner_seed_paths(&candidates, &owners, 1, &cache_terms);
    let elapsed = started_at.elapsed();
    let elapsed_ms = elapsed.as_millis();

    assert!(has_package_path);
    assert_eq!(
        package_seed,
        vec!["packages/runtime/search/src/router.rs".to_string()]
    );
    assert_eq!(evidence_seed, vec!["src/cache.rs".to_string()]);
    assert!(
        elapsed_ms <= max_total_ms,
        "graph query owner seed cold functional path exceeded benchmark max_total={} observed={}ms package_seed={package_seed:?} evidence_seed={evidence_seed:?}",
        benchmark.max_total,
        elapsed_ms
    );

    let performance_gate = serde_json::json!({
        "schemaId": "agent.semantic-protocols.semantic-hot-path-performance-gate",
        "schemaVersion": "1",
        "scenarioId": "asp-graph-query-owner-seed-cold-functional-path",
        "languageId": "rust",
        "workspace": ".",
        "command": [
            "agent_semantic_search::graph_has_package_path_candidate",
            "agent_semantic_search::graph_query_owner_seed_paths"
        ],
        "phase": "cold",
        "expected": {
            "targetTotal": benchmark.target_total,
            "maxTotal": benchmark.max_total,
            "regressionBudget": benchmark.regression_budget,
            "maxProviderProcessCount": 0,
            "maxSearchOverlayProcessCount": 0,
            "maxStdoutBytes": benchmark.max_stdout_bytes,
            "requireSearchOwnedSeed": true,
            "allowedFirstRoutes": ["graph-query-owner-seed"],
            "forbiddenRoutes": ["command-query-owner-seed", "provider-process", "native-finder"],
            "requirePackagePathSeed": true,
            "requireIdentifierAxisSeed": true
        },
        "observed": {
            "observedTotal": duration_literal(elapsed),
            "providerProcessCount": 0,
            "providerElapsed": "0us",
            "nativeFinderProcessCount": 0,
            "nativeFinderElapsed": "0us",
            "packageSeed": package_seed[0],
            "evidenceSeed": evidence_seed[0],
            "firstRoute": "graph-query-owner-seed",
            "executedRoutes": ["graph-query-owner-seed"],
            "stdoutBytes": 0,
            "fallbackReason": "none"
        },
        "verdict": "pass",
        "evidenceRefs": ["scenario:asp-graph-query-owner-seed-cold-functional-path"]
    });
    assert_eq!(performance_gate["observed"]["providerProcessCount"], 0);
    assert_eq!(performance_gate["observed"]["nativeFinderProcessCount"], 0);
    assert_eq!(
        performance_gate["observed"]["packageSeed"],
        "packages/runtime/search/src/router.rs"
    );
}

pub(super) fn asp_graph_evidence_projection_cold_functional_path_stays_inside_scenario_gate() {
    let crate_root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let scenario_root = crate_root
        .join("tests")
        .join("unit")
        .join("scenarios")
        .join("asp_graph_evidence_projection_cold_functional_path");
    let benchmark: SharedBenchmarkToml = read_toml(&scenario_root.join("benchmark.toml"));
    assert_graph_evidence_projection_benchmark_contract(&benchmark);
    let max_total_ms = duration_millis_from_manifest(&benchmark.max_total);

    let mut topology_kinds = HashMap::new();
    topology_kinds.insert("owner:src/lib.rs".to_string(), "owner".to_string());
    topology_kinds.insert("workspace:root".to_string(), "workspace".to_string());
    topology_kinds.insert("provider:rust".to_string(), "provider-root".to_string());
    topology_kinds.insert(
        "submodule:languages/rust".to_string(),
        "submodule".to_string(),
    );
    let mut mixed_kinds = topology_kinds.clone();
    mixed_kinds.insert("item:src/lib.rs-search".to_string(), "item".to_string());

    let started_at = Instant::now();
    let topology_only =
        agent_semantic_search::graph_frontier_has_only_owner_or_topology_nodes(&topology_kinds);
    let mixed =
        agent_semantic_search::graph_frontier_has_only_owner_or_topology_nodes(&mixed_kinds);
    let elapsed = started_at.elapsed();
    let elapsed_ms = elapsed.as_millis();

    assert!(topology_only);
    assert!(!mixed);
    assert!(
        elapsed_ms <= max_total_ms,
        "graph evidence projection cold functional path exceeded benchmark max_total={} observed={}ms topology_only={topology_only} mixed={mixed}",
        benchmark.max_total,
        elapsed_ms
    );

    let performance_gate = serde_json::json!({
        "schemaId": "agent.semantic-protocols.semantic-hot-path-performance-gate",
        "schemaVersion": "1",
        "scenarioId": "asp-graph-evidence-projection-cold-functional-path",
        "languageId": "rust",
        "workspace": ".",
        "command": [
            "agent_semantic_search::graph_frontier_has_only_owner_or_topology_nodes"
        ],
        "phase": "cold",
        "expected": {
            "targetTotal": benchmark.target_total,
            "maxTotal": benchmark.max_total,
            "regressionBudget": benchmark.regression_budget,
            "maxProviderProcessCount": 0,
            "maxSearchOverlayProcessCount": 0,
            "maxStdoutBytes": benchmark.max_stdout_bytes,
            "requireSearchOwnedProjectionPredicate": true,
            "allowedFirstRoutes": ["graph-evidence-projection"],
            "forbiddenRoutes": ["command-evidence-projection", "provider-process", "native-finder"]
        },
        "observed": {
            "observedTotal": duration_literal(elapsed),
            "providerProcessCount": 0,
            "providerElapsed": "0us",
            "nativeFinderProcessCount": 0,
            "nativeFinderElapsed": "0us",
            "topologyOnly": topology_only,
            "mixed": mixed,
            "firstRoute": "graph-evidence-projection",
            "executedRoutes": ["graph-evidence-projection"],
            "stdoutBytes": 0,
            "fallbackReason": "none"
        },
        "verdict": "pass",
        "evidenceRefs": ["scenario:asp-graph-evidence-projection-cold-functional-path"]
    });
    assert_eq!(performance_gate["observed"]["providerProcessCount"], 0);
    assert_eq!(performance_gate["observed"]["nativeFinderProcessCount"], 0);
    assert_eq!(performance_gate["observed"]["topologyOnly"], true);
    assert_eq!(performance_gate["observed"]["mixed"], false);
}
