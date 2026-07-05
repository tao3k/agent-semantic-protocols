use crate::graph_seed_decision::{
    SearchActionSelection, SearchEvidenceState, SeedActionIntent, SeedPhaseDecision,
    recommended_action_for_seed_risk,
};

#[test]
fn graph_seed_decision_projects_owner_anchor_budget_from_query_shape() {
    let broad = SeedPhaseDecision::from_query_shape(true, 6, 4);

    assert_eq!(broad.query_owner_anchor_budget, 2);
    assert_eq!(broad.risk_factors, ["flat-query", "owner-drift"]);

    let single_owner = SeedPhaseDecision::from_query_shape(true, 6, 1);
    assert_eq!(single_owner.query_owner_anchor_budget, 0);
    assert_eq!(single_owner.risk_factors, ["flat-query"]);
}

#[test]
fn graph_seed_decision_keeps_seed_actions_typed() {
    assert_eq!(
        SeedActionIntent::from_seed_plan_action("split-query-pack"),
        Some(SeedActionIntent::SplitQueryPack)
    );
    assert_eq!(
        SeedActionIntent::from_seed_plan_action("narrow-owner-scope"),
        Some(SeedActionIntent::NarrowOwnerScope)
    );
    assert_eq!(
        SeedActionIntent::from_seed_plan_action("keep-query-seed"),
        None
    );
    assert_eq!(
        recommended_action_for_seed_risk("flat-query"),
        Some("split-query-pack")
    );
    assert_eq!(
        recommended_action_for_seed_risk("owner-drift"),
        Some("narrow-owner-scope")
    );
    assert_eq!(recommended_action_for_seed_risk("unknown"), None);
}

#[test]
fn graph_seed_decision_maps_evidence_state_to_first_action_preconditions() {
    let unknown = SearchActionSelection::for_first_action(SearchEvidenceState::Unknown, "seed");

    assert!(unknown.first_action_matches_evidence_state);
    assert!(unknown.reasoning_tree_route_shown);
    assert!(unknown.chosen_route_preconditions_met);
    assert_eq!(unknown.unnecessary_seed_count, 0);

    let owner = SearchActionSelection::for_first_action(SearchEvidenceState::KnownOwner, "seed");
    assert!(!owner.first_action_matches_evidence_state);
    assert_eq!(owner.unnecessary_seed_count, 1);
    assert_eq!(owner.seed_when_known_owner_count, 1);
    assert_eq!(owner.seed_when_known_symbol_count, 0);
    assert_eq!(owner.seed_when_known_selector_count, 0);
    assert!(owner.allowed_first_stages.contains(&"owner-items"));

    let symbol = SearchActionSelection::for_first_action(SearchEvidenceState::KnownSymbol, "seed");
    assert!(!symbol.first_action_matches_evidence_state);
    assert_eq!(symbol.seed_when_known_symbol_count, 1);

    let selector =
        SearchActionSelection::for_first_action(SearchEvidenceState::KnownSelector, "seed");
    assert!(!selector.first_action_matches_evidence_state);
    assert_eq!(selector.seed_when_known_selector_count, 1);
    assert!(selector.disallowed_first_stages.contains(&"broad-rg"));
}

#[test]
fn graph_seed_decision_projects_graph_turbo_seed_plan_packet() {
    let seed_ids = vec![
        "query:search-router".to_string(),
        "owner:src/router.rs".to_string(),
    ];
    let seed_decision = SeedPhaseDecision::from_query_shape(true, 6, 4);

    let seed_plan = crate::graph_turbo_seed_plan(crate::GraphTurboSeedPlanInput {
        query_present: true,
        query_seed_present: true,
        candidate_count: 9,
        candidate_owner_count: 4,
        query_owner_seed_count: 1,
        fallback_owner_seed_count: 0,
        seed_ids: &seed_ids,
        seed_decision: &seed_decision,
    });

    assert_eq!(seed_plan["phase"], "seed-query");
    assert_eq!(seed_plan["reason"], "query");
    assert_eq!(seed_plan["seedQuality"], "review");
    assert_eq!(seed_plan["selectedSeedCount"], 2);
    assert_eq!(
        seed_plan["riskFactors"],
        serde_json::json!(["flat-query", "owner-drift"])
    );
    assert_eq!(
        seed_plan["recommendedActions"],
        serde_json::json!(["split-query-pack", "narrow-owner-scope"])
    );
    assert_eq!(
        seed_plan["selectionPolicy"]["flow"],
        "evidence-state-reasoning-tree"
    );
    assert_eq!(seed_plan["selectionPolicy"]["firstActionStage"], "seed");
}

#[test]
fn graph_seed_decision_lists_all_reasoning_tree_inputs() {
    let states = SearchEvidenceState::all()
        .iter()
        .map(|state| state.as_str())
        .collect::<Vec<_>>();

    assert_eq!(
        states,
        [
            "unknown",
            "known-owner",
            "known-symbol",
            "known-selector",
            "known-dependency",
            "known-changed-file",
            "known-failure",
        ]
    );
    assert!(
        SearchEvidenceState::KnownDependency
            .allowed_first_stages()
            .contains(&"dependency-topology")
    );
    assert!(
        SearchEvidenceState::KnownChangedFile
            .allowed_first_stages()
            .contains(&"owner-skeleton")
    );
    assert!(
        SearchEvidenceState::KnownFailure
            .allowed_first_stages()
            .contains(&"failure-frontier")
    );
}
