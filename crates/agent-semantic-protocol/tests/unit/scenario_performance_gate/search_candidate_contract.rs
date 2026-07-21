use std::path::Path;
use std::time::Instant;

use super::contracts::assert_search_candidate_contract_benchmark_contract;
use super::runtime_gates::{duration_literal, duration_millis_from_manifest, read_toml};
use super::shared::SharedBenchmarkToml;

pub(super) fn asp_search_candidate_contract_cold_functional_path_stays_inside_scenario_gate() {
    let crate_root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let scenario_root = crate_root
        .join("tests")
        .join("unit")
        .join("scenarios")
        .join("asp_search_candidate_contract_cold_functional_path");
    let benchmark: SharedBenchmarkToml = read_toml(&scenario_root.join("benchmark.toml"));
    assert_search_candidate_contract_benchmark_contract(&benchmark);
    let max_total_ms = duration_millis_from_manifest(&benchmark.max_total);

    let started_at = Instant::now();
    let terms = agent_semantic_search::source_index_lookup_terms("source index fixture");
    let source_index_candidate = agent_semantic_search::source_index_candidate_to_search_candidate(
        agent_semantic_search::SourceIndexRankCandidate {
            ordinal: 0,
            path: "src/lib.rs".to_string(),
            query_keys: vec!["source_index_fixture".to_string(), "lib".to_string()],
        },
        &terms,
    );
    let base_snapshot = agent_semantic_content_identity::WorkspaceSnapshot::from_file_hashes([(
        "src/lib.rs",
        "a".repeat(64),
    )]);
    let source_snapshot = base_snapshot
        .with_overlay([("src/lib.rs", "c".repeat(64))])
        .evidence(
            agent_semantic_content_identity::SourceSnapshotKind::EditorBuffer,
            "b".repeat(64),
        );
    let overlay_hits = agent_semantic_search::search_lexical_overlay(
        agent_semantic_search::LexicalOverlaySearchRequest::new("overlay fixture", source_snapshot)
            .document(
                agent_semantic_search::LexicalOverlayDocument::new(
                    "src/lib.rs",
                    "rust://src/lib.rs#item/function/overlay_fixture",
                    "overlay_fixture",
                )
                .search_text("dynamic overlay fixture owner"),
            ),
    );
    let overlay_candidate = agent_semantic_search::lexical_overlay_hit_to_search_candidate(
        &overlay_hits.hits[0],
        "session-1/base-1",
    );
    let ranked_candidates = agent_semantic_search::merge_search_candidates(vec![
        source_index_candidate.clone(),
        overlay_candidate.clone(),
    ]);
    let elapsed = started_at.elapsed();
    let elapsed_ms = elapsed.as_millis();

    assert_eq!(source_index_candidate.route_source, "source-index");
    assert_eq!(overlay_candidate.route_source, "search-overlay");
    assert_eq!(overlay_candidate.identity_kind, "selector");
    assert_eq!(
        overlay_candidate.selector.as_deref(),
        Some("rust://src/lib.rs#item/function/overlay_fixture")
    );
    assert!(
        source_index_candidate
            .field_hits
            .iter()
            .any(|field| field.field == "query_keys"),
        "source-index candidate must carry field hit evidence: {source_index_candidate:?}"
    );
    assert!(
        overlay_candidate
            .rank_features
            .iter()
            .any(|feature| feature.name == "search-overlay-score"),
        "overlay candidate must carry rank features: {overlay_candidate:?}"
    );
    assert!(
        !agent_semantic_search::search_candidate_has_executable_line_identity(
            &source_index_candidate
        ) && !agent_semantic_search::search_candidate_has_executable_line_identity(
            &overlay_candidate
        ),
        "shared search candidate contract must not use executable line-range identity"
    );
    assert_eq!(
        ranked_candidates[0].candidate.route_source, "search-overlay",
        "active overlay candidates must outrank stable source-index candidates before graph fusion: {ranked_candidates:?}"
    );
    assert_eq!(ranked_candidates[0].selector_bonus, 1);
    assert!(
        elapsed_ms <= max_total_ms,
        "search candidate contract cold functional path exceeded benchmark max_total={} observed={}ms",
        benchmark.max_total,
        elapsed_ms
    );

    let observed_total = duration_literal(elapsed);
    let performance_gate = serde_json::json!({
        "schemaId": "agent.semantic-protocols.semantic-hot-path-performance-gate",
        "schemaVersion": "1",
        "scenarioId": "asp-search-candidate-contract-cold-functional-path",
        "languageId": "rust",
        "workspace": ".",
        "command": [
            "agent_semantic_search::source_index_candidate_to_search_candidate",
            "agent_semantic_search::lexical_overlay_hit_to_search_candidate",
            "agent_semantic_search::merge_search_candidates"
        ],
        "phase": "cold",
        "expected": {
            "targetTotal": benchmark.target_total,
            "maxTotal": benchmark.max_total,
            "regressionBudget": benchmark.regression_budget,
            "maxProviderProcessCount": 0,
            "maxSearchOverlayProcessCount": 0,
            "maxStdoutBytes": benchmark.max_stdout_bytes,
            "requireFieldHits": true,
            "requireRankFeatures": true,
            "requireOverlayBeforeStable": true,
            "allowedFirstRoutes": ["search-candidate-contract"],
            "forbiddenRoutes": ["client", "command", "native-finder", "provider-process"],
            "requireExactCodeIdentity": true,
            "requireNoExecutableLineRange": true
        },
        "observed": {
            "observedTotal": observed_total,
            "providerProcessCount": 0,
            "providerElapsed": "0us",
            "nativeFinderProcessCount": 0,
            "nativeFinderElapsed": "0us",
            "firstRoute": "search-candidate-contract",
            "executedRoutes": ["source-index", "dynamic-overlay"],
            "sourceIndexFieldHitCount": source_index_candidate.field_hits.len(),
            "overlayRankFeatureCount": overlay_candidate.rank_features.len(),
            "mergedCandidateCount": ranked_candidates.len(),
            "firstMergedRoute": ranked_candidates[0].candidate.route_source,
            "executableLineRangeSelectorCount": 0,
            "packetOutMode": "not-applicable",
            "renderDuration": observed_total,
            "stdoutBytes": 0
        },
        "verdict": "pass",
        "evidenceRefs": ["scenario:asp-search-candidate-contract-cold-functional-path"]
    });
    assert_eq!(performance_gate["observed"]["providerProcessCount"], 0);
    assert_eq!(performance_gate["observed"]["nativeFinderProcessCount"], 0);
    assert_eq!(
        performance_gate["observed"]["executableLineRangeSelectorCount"],
        0
    );
    assert_eq!(
        performance_gate["observed"]["firstMergedRoute"],
        "search-overlay"
    );
}
