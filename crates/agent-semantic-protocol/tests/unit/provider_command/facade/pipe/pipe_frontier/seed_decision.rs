#[path = "../../../../../../src/command/search_pipe_seed_decision.rs"]
mod search_pipe_seed_decision;

use search_pipe_seed_decision::{
    SeedActionIntent, SeedPhaseDecision, recommended_action_for_seed_risk,
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
