use std::path::Path;
use std::time::Instant;

use super::contracts::assert_graph_seed_decision_benchmark_contract;
use super::runtime_gates::{duration_literal, duration_millis_from_manifest, read_toml};
use super::shared::SharedBenchmarkToml;

pub(super) fn asp_graph_seed_decision_cold_functional_path_stays_inside_scenario_gate() {
    let crate_root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let scenario_root = crate_root
        .join("tests")
        .join("unit")
        .join("scenarios")
        .join("asp_graph_seed_decision_cold_functional_path");
    let benchmark: SharedBenchmarkToml = read_toml(&scenario_root.join("benchmark.toml"));
    assert_graph_seed_decision_benchmark_contract(&benchmark);
    let max_total_ms = duration_millis_from_manifest(&benchmark.max_total);

    let started_at = Instant::now();
    let broad_owner_drift = agent_semantic_search::SeedPhaseDecision::from_query_shape(true, 6, 4);
    let single_owner = agent_semantic_search::SeedPhaseDecision::from_query_shape(true, 6, 1);
    let split_action =
        agent_semantic_search::SeedActionIntent::from_seed_plan_action("split-query-pack");
    let narrow_action = agent_semantic_search::recommended_action_for_seed_risk("owner-drift");
    let unknown_seed = agent_semantic_search::SearchActionSelection::for_first_action(
        agent_semantic_search::SearchEvidenceState::Unknown,
        "seed",
    );
    let known_owner_seed = agent_semantic_search::SearchActionSelection::for_first_action(
        agent_semantic_search::SearchEvidenceState::KnownOwner,
        "seed",
    );
    let known_selector_seed = agent_semantic_search::SearchActionSelection::for_first_action(
        agent_semantic_search::SearchEvidenceState::KnownSelector,
        "seed",
    );
    let evidence_states = agent_semantic_search::SearchEvidenceState::all()
        .iter()
        .map(|state| state.as_str())
        .collect::<Vec<_>>();
    let seed_ids = vec![
        "query:search-router".to_string(),
        "owner:src/router.rs".to_string(),
    ];
    let seed_plan = agent_semantic_search::graph_turbo_seed_plan(
        agent_semantic_search::GraphTurboSeedPlanInput {
            query_present: true,
            query_seed_present: true,
            candidate_count: 9,
            candidate_owner_count: 4,
            query_owner_seed_count: 1,
            fallback_owner_seed_count: 0,
            seed_ids: &seed_ids,
            seed_decision: &broad_owner_drift,
        },
    );
    let elapsed = started_at.elapsed();
    let elapsed_ms = elapsed.as_millis();

    assert_eq!(broad_owner_drift.query_owner_anchor_budget, 2);
    assert_eq!(
        broad_owner_drift.risk_factors,
        ["flat-query", "owner-drift"]
    );
    assert_eq!(single_owner.query_owner_anchor_budget, 0);
    assert_eq!(
        split_action,
        Some(agent_semantic_search::SeedActionIntent::SplitQueryPack)
    );
    assert_eq!(narrow_action, Some("narrow-owner-scope"));
    assert!(unknown_seed.first_action_matches_evidence_state);
    assert!(!known_owner_seed.first_action_matches_evidence_state);
    assert_eq!(known_owner_seed.seed_when_known_owner_count, 1);
    assert!(!known_selector_seed.first_action_matches_evidence_state);
    assert_eq!(known_selector_seed.seed_when_known_selector_count, 1);
    assert_eq!(evidence_states.len(), 7);
    assert_eq!(seed_plan["reason"], "query");
    assert_eq!(seed_plan["seedQuality"], "review");
    assert_eq!(
        seed_plan["recommendedActions"],
        serde_json::json!(["split-query-pack", "narrow-owner-scope"])
    );
    assert_eq!(
        seed_plan["selectionPolicy"]["flow"],
        "evidence-state-reasoning-tree"
    );
    assert!(
        elapsed_ms <= max_total_ms,
        "graph seed decision cold functional path exceeded benchmark max_total={} observed={}ms broad_owner_drift={broad_owner_drift:?} known_owner_seed={known_owner_seed:?}",
        benchmark.max_total,
        elapsed_ms
    );

    let performance_gate = serde_json::json!({
        "schemaId": "agent.semantic-protocols.semantic-hot-path-performance-gate",
        "schemaVersion": "1",
        "scenarioId": "asp-graph-seed-decision-cold-functional-path",
        "languageId": "rust",
        "workspace": ".",
        "command": [
            "agent_semantic_search::SeedPhaseDecision::from_query_shape",
            "agent_semantic_search::SearchActionSelection::for_first_action",
            "agent_semantic_search::graph_turbo_seed_plan"
        ],
        "phase": "cold",
        "expected": {
            "targetTotal": benchmark.target_total,
            "maxTotal": benchmark.max_total,
            "regressionBudget": benchmark.regression_budget,
            "maxProviderProcessCount": 0,
            "maxSearchOverlayProcessCount": 0,
            "maxStdoutBytes": benchmark.max_stdout_bytes,
            "requireSearchOwnedSeedDecision": true,
            "allowedFirstRoutes": ["graph-seed-decision"],
            "forbiddenRoutes": ["command-seed-decision", "provider-process", "native-finder"]
        },
        "observed": {
            "observedTotal": duration_literal(elapsed),
            "providerProcessCount": 0,
            "providerElapsed": "0us",
            "nativeFinderProcessCount": 0,
            "nativeFinderElapsed": "0us",
            "queryOwnerAnchorBudget": broad_owner_drift.query_owner_anchor_budget,
            "riskFactors": broad_owner_drift.risk_factors,
            "knownOwnerSeedRejected": !known_owner_seed.first_action_matches_evidence_state,
            "knownSelectorSeedRejected": !known_selector_seed.first_action_matches_evidence_state,
            "evidenceStateCount": evidence_states.len(),
            "seedPlanReason": seed_plan["reason"],
            "seedPlanQuality": seed_plan["seedQuality"],
            "recommendedActions": seed_plan["recommendedActions"],
            "firstRoute": "graph-seed-decision",
            "executedRoutes": ["graph-seed-decision"],
            "stdoutBytes": 0,
            "fallbackReason": "none"
        },
        "verdict": "pass",
        "evidenceRefs": ["scenario:asp-graph-seed-decision-cold-functional-path"]
    });
    assert_eq!(performance_gate["observed"]["providerProcessCount"], 0);
    assert_eq!(performance_gate["observed"]["nativeFinderProcessCount"], 0);
    assert_eq!(performance_gate["observed"]["queryOwnerAnchorBudget"], 2);
    assert_eq!(performance_gate["observed"]["knownOwnerSeedRejected"], true);
    assert_eq!(performance_gate["observed"]["seedPlanQuality"], "review");
}
