use agent_semantic_search::{
    SearchActionSelection, SearchEvidenceState, SeedActionIntent, SeedPhaseDecision,
    recommended_action_for_seed_risk,
};

#[test]
fn broad_query_with_owner_drift_gets_owner_anchor_budget() {
    let decision = SeedPhaseDecision::from_query_shape(true, 6, 4);

    assert_eq!(decision.query_owner_anchor_budget, 2);
    assert_eq!(decision.risk_factors, ["flat-query", "owner-drift"]);
}

#[test]
fn single_owner_query_keeps_anchor_budget_empty() {
    let decision = SeedPhaseDecision::from_query_shape(true, 6, 1);

    assert_eq!(decision.query_owner_anchor_budget, 0);
    assert_eq!(decision.risk_factors, ["flat-query"]);
}

#[test]
fn seed_plan_actions_are_typed_intents() {
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
}

#[test]
fn seed_risks_project_to_report_actions() {
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
fn unknown_evidence_allows_seed_as_orientation_route() {
    let selection = SearchActionSelection::for_first_action(SearchEvidenceState::Unknown, "seed");

    assert!(selection.first_action_matches_evidence_state);
    assert!(selection.reasoning_tree_route_shown);
    assert!(selection.chosen_route_preconditions_met);
    assert_eq!(selection.unnecessary_seed_count, 0);
}

#[test]
fn evidence_state_table_lists_all_reasoning_tree_inputs() {
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

#[test]
fn known_owner_symbol_and_selector_reject_seed_as_first_action() {
    let owner_selection =
        SearchActionSelection::for_first_action(SearchEvidenceState::KnownOwner, "seed");
    assert!(!owner_selection.first_action_matches_evidence_state);
    assert_eq!(owner_selection.unnecessary_seed_count, 1);
    assert_eq!(owner_selection.seed_when_known_owner_count, 1);
    assert_eq!(owner_selection.seed_when_known_symbol_count, 0);
    assert_eq!(owner_selection.seed_when_known_selector_count, 0);
    assert!(
        owner_selection
            .allowed_first_stages
            .contains(&"owner-items")
    );

    let symbol_selection =
        SearchActionSelection::for_first_action(SearchEvidenceState::KnownSymbol, "seed");
    assert!(!symbol_selection.first_action_matches_evidence_state);
    assert_eq!(symbol_selection.seed_when_known_symbol_count, 1);
    assert!(
        symbol_selection
            .allowed_first_stages
            .contains(&"item-skeleton")
    );

    let selector_selection =
        SearchActionSelection::for_first_action(SearchEvidenceState::KnownSelector, "seed");
    assert!(!selector_selection.first_action_matches_evidence_state);
    assert_eq!(selector_selection.seed_when_known_selector_count, 1);
    assert!(
        selector_selection
            .allowed_first_stages
            .contains(&"query-code")
    );
    assert!(
        selector_selection
            .disallowed_first_stages
            .contains(&"fd-query")
    );
}
