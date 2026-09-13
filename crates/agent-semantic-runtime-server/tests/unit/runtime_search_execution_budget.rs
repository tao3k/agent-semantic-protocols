// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use super::RuntimeSearchExecutionBudget;

#[test]
fn budget_is_derived_from_generation_cardinality_and_protocol_envelope() {
    let budget = RuntimeSearchExecutionBudget::from_cardinalities(
        &format!("blake3-256:{}", "a".repeat(64)),
        700,
        90_000,
        8_000,
        600,
        4_000,
    )
    .expect("derive budget");

    assert_eq!(budget.rg_match_limit(), 8_000);
    assert_eq!(budget.lexical_owner_limit(), 700);
    assert_eq!(budget.syntax_selector_limit(), 90_000);
    assert_eq!(budget.graph_candidate_owner_limit(), 700);
    assert_eq!(
        budget.graph_evaluation_budget(),
        Some(agent_semantic_search::ResidentGraphEvaluationBudget {
            max_depth: 16,
            max_nodes: 256,
            max_edges: 1024,
            max_results: 30,
        })
    );
    assert_eq!(budget.evidence_item_limit(), 30);
    let receipt = serde_json::to_value(&budget).expect("serialize typed budget receipt");
    assert_eq!(
        receipt["schemaId"],
        "agent.semantic-protocols.runtime-search-execution-budget"
    );
    assert_eq!(receipt["authority"], "runtime-generation");
    assert_eq!(receipt["cardinality"]["corpusLineCount"], 8_000);
    assert_eq!(receipt["limits"]["rgMatchCount"], 8_000);
}

#[test]
fn ready_empty_graph_has_no_evaluator_budget() {
    let budget = RuntimeSearchExecutionBudget::from_cardinalities(
        &format!("blake3-256:{}", "c".repeat(64)),
        4,
        100,
        10,
        4,
        0,
    )
    .expect("derive Ready-empty Graph budget");

    assert_eq!(budget.graph_evaluation_budget(), None);
}

#[test]
fn budget_rejects_empty_or_unrepresentable_generation_cardinality() {
    let digest = format!("blake3-256:{}", "b".repeat(64));
    assert!(RuntimeSearchExecutionBudget::from_cardinalities(&digest, 0, 1, 1, 0, 0).is_err());
    assert!(
        RuntimeSearchExecutionBudget::from_cardinalities(
            &digest,
            1,
            1,
            u64::from(u32::MAX) + 1,
            0,
            0,
        )
        .is_err()
    );
}
