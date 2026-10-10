// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use agent_semantic_mrr::{
    ProjectTopologyReasoningInputV1, ProjectTopologyReasoningLimitsV1, evaluate_project_topology_v1,
};

fn edge(id: &str, relation: &str, from: &str, to: &str) -> ProjectTopologyReasoningInputV1 {
    ProjectTopologyReasoningInputV1 {
        fact_id: id.to_owned(),
        relation: relation.to_owned(),
        from: from.to_owned(),
        to: to.to_owned(),
    }
}

fn limits() -> ProjectTopologyReasoningLimitsV1 {
    ProjectTopologyReasoningLimitsV1 {
        max_input_facts: 16,
        max_candidates: 16,
    }
}

#[test]
fn relation_sensitive_ascent_join_matches_the_lean_domain_rule() {
    let receipt = evaluate_project_topology_v1(
        &[
            edge("w1", "CALLS", "refresh", "publish"),
            edge("w2", "READS_CONFIG", "publish", "hook"),
            edge("w3", "COVERS", "test", "publish"),
        ],
        limits(),
    )
    .expect("complete relation-sensitive closure");

    let [candidate] = receipt.candidates() else {
        panic!("one config dependency must be derived")
    };
    assert_eq!(candidate.relation(), "DEPENDS_ON_CONFIG");
    assert_eq!(candidate.from(), "refresh");
    assert_eq!(candidate.to(), "hook");
    assert_eq!(candidate.premise_fact_ids(), ["w1", "w2"]);
    assert!(receipt.digest().starts_with("blake3-256:"));
}

#[test]
fn covers_cannot_impersonate_reads_config() {
    let receipt = evaluate_project_topology_v1(
        &[
            edge("w1", "CALLS", "refresh", "publish"),
            edge("w2", "COVERS", "publish", "hook"),
        ],
        limits(),
    )
    .expect("complete empty relation-sensitive closure");

    assert!(receipt.candidates().is_empty());
}

#[test]
fn noncanonical_relation_cannot_enter_the_ascent_edb() {
    let error = evaluate_project_topology_v1(
        &[
            edge("w1", "calls", "refresh", "publish"),
            edge("w2", "READS_CONFIG", "publish", "hook"),
        ],
        limits(),
    )
    .expect_err("source relations must be canonical before Ascent evaluation");

    assert!(error.to_string().contains("canonical uppercase relations"));
}

#[test]
fn candidate_budget_fails_closed() {
    let error = evaluate_project_topology_v1(
        &[
            edge("w1", "CALLS", "refresh", "publish"),
            edge("w2", "READS_CONFIG", "publish", "hook"),
        ],
        ProjectTopologyReasoningLimitsV1 {
            max_input_facts: 2,
            max_candidates: 0,
        },
    )
    .expect_err("zero candidate budget is not a fixed point receipt");

    assert!(error.to_string().contains("limits must be non-zero"));
}
