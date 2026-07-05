use std::path::Path;
use std::time::Instant;

use super::contracts::assert_evidence_graph_rank_benchmark_contract;
use super::runtime_gates::{duration_literal, duration_millis_from_manifest, read_toml};
use super::shared::SharedBenchmarkToml;

pub(super) fn asp_evidence_graph_rank_cold_functional_path_stays_inside_scenario_gate() {
    let crate_root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let scenario_root = crate_root
        .join("tests")
        .join("unit")
        .join("scenarios")
        .join("asp_evidence_graph_rank_cold_functional_path");
    let benchmark: SharedBenchmarkToml = read_toml(&scenario_root.join("benchmark.toml"));
    assert_evidence_graph_rank_benchmark_contract(&benchmark);
    let max_total_ms = duration_millis_from_manifest(&benchmark.max_total);

    let nodes = vec![
        agent_semantic_search::EvidenceGraphRankNode {
            ordinal: 0,
            id: "structural-owner:generation:src/lib.rs".to_string(),
            kind: "structural-owner".to_string(),
            label: "src/lib.rs".to_string(),
            path: Some("src/lib.rs".to_string()),
            selector: None,
            query_keys: vec!["lib".to_string()],
            outgoing_edge_count: 8,
        },
        agent_semantic_search::EvidenceGraphRankNode {
            ordinal: 1,
            id: "selector:rust://src/lib.rs#item/struct/EvidenceFixture".to_string(),
            kind: "selector".to_string(),
            label: "EvidenceFixture".to_string(),
            path: Some("src/lib.rs".to_string()),
            selector: Some("rust://src/lib.rs#item/struct/EvidenceFixture".to_string()),
            query_keys: vec!["EvidenceFixture".to_string(), "serde".to_string()],
            outgoing_edge_count: 0,
        },
        agent_semantic_search::EvidenceGraphRankNode {
            ordinal: 2,
            id: "symbol:rust://src/lib.rs#item/impl/Serialize".to_string(),
            kind: "symbol".to_string(),
            label: "Serialize impl".to_string(),
            path: Some("src/lib.rs".to_string()),
            selector: Some("rust://src/lib.rs#item/impl/Serialize".to_string()),
            query_keys: vec!["serde".to_string()],
            outgoing_edge_count: 2,
        },
    ];
    assert!(
        nodes.iter().all(|node| !node.id.contains(":1:")
            && !node.selector.as_deref().unwrap_or("").contains(":1:")),
        "EvidenceGraph rank nodes must not encode executable line ranges"
    );

    let started_at = Instant::now();
    let ranked = agent_semantic_search::rank_evidence_graph_nodes(nodes, "serde EvidenceFixture");
    let elapsed = started_at.elapsed();
    let elapsed_ms = elapsed.as_millis();
    assert_eq!(
        ranked[0].node.selector.as_deref(),
        Some("rust://src/lib.rs#item/struct/EvidenceFixture")
    );
    assert_eq!(ranked[0].score.term_hits, 2);
    assert_eq!(ranked[0].score.selector_bonus, 1);
    assert!(
        ranked.iter().all(|ranked| ranked.score.topology_bonus <= 8),
        "topology bonus must stay bounded; ranked={ranked:?}"
    );
    assert!(
        elapsed_ms <= max_total_ms,
        "EvidenceGraph rank cold functional path exceeded benchmark max_total={} observed={}ms ranked={ranked:?}",
        benchmark.max_total,
        elapsed_ms
    );

    let observed_total = duration_literal(elapsed);
    let performance_gate = serde_json::json!({
        "schemaId": "agent.semantic-protocols.semantic-hot-path-performance-gate",
        "schemaVersion": "1",
        "scenarioId": "asp-evidence-graph-rank-cold-functional-path",
        "languageId": "rust",
        "workspace": ".",
        "command": [
            "agent_semantic_search::rank_evidence_graph_nodes"
        ],
        "phase": "cold",
        "expected": {
            "targetTotal": benchmark.target_total,
            "maxTotal": benchmark.max_total,
            "regressionBudget": benchmark.regression_budget,
            "maxProviderProcessCount": 0,
            "maxSearchOverlayProcessCount": 0,
            "maxStdoutBytes": benchmark.max_stdout_bytes,
            "requireSelectorFirst": true,
            "requireBoundedTopologyBonus": true,
            "requireNoExecutableLineRange": true
        },
        "observed": {
            "observedTotal": observed_total,
            "providerProcessCount": 0,
            "nativeFinderProcessCount": 0,
            "firstRoute": benchmark.route_source,
            "executedRoutes": [benchmark.route_source],
            "selectorFirst": true,
            "maxTopologyBonus": ranked.iter().map(|node| node.score.topology_bonus).max().unwrap_or(0),
            "executableLineRangeSelectorCount": 0,
            "stdoutBytes": 0,
            "fallbackReason": benchmark.fallback_reason
        },
        "verdict": "pass",
        "evidenceRefs": ["scenario:asp-evidence-graph-rank-cold-functional-path"]
    });
    assert_eq!(performance_gate["observed"]["providerProcessCount"], 0);
    assert_eq!(performance_gate["observed"]["nativeFinderProcessCount"], 0);
    assert_eq!(performance_gate["observed"]["selectorFirst"], true);
}
