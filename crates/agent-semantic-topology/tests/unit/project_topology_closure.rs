// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use std::sync::Arc;

use agent_semantic_topology::{
    ProjectTopologyClosureBuilder, ProjectTopologyClosureLimits, ProjectTopologyDirectEdge,
    ProjectTopologyInferenceProgram,
};

fn edge(id: &str, relation: &str, from: &str, to: &str) -> ProjectTopologyDirectEdge {
    ProjectTopologyDirectEdge::new(id, relation, from, to)
        .expect("valid parser-owned topology edge")
}

fn limits() -> ProjectTopologyClosureLimits {
    ProjectTopologyClosureLimits::new(16, 64, 64).expect("non-zero bounded limits")
}

fn builder(limits: ProjectTopologyClosureLimits) -> ProjectTopologyClosureBuilder {
    ProjectTopologyClosureBuilder::new(
        limits,
        Arc::new(ProjectTopologyInferenceProgram::standard().expect("standard MRR program")),
    )
}

#[test]
fn topology_closure_is_deterministic_and_carries_complete_mrr_receipt() {
    let edges = vec![
        edge("declares-a-b", "DECLARES", "a", "b"),
        edge("calls-b-c", "CALLS", "b", "c"),
    ];
    let builder = builder(limits());

    let first = builder
        .build("topology-generation-1", &edges)
        .expect("complete topology closure");
    let second = builder
        .build("topology-generation-1", &edges)
        .expect("same complete topology closure");

    assert_eq!(first, second);
    let transitive = first
        .relationships()
        .iter()
        .find(|relationship| relationship.from() == "a" && relationship.to() == "c")
        .expect("Ascent derives the transitive topology path");
    assert_eq!(transitive.premise_edge_ids(), ["calls-b-c", "declares-a-b"]);
    assert_eq!(first.receipt().state(), "admitted");
    assert_eq!(first.receipt().input_edge_count(), 2);
    assert_eq!(first.receipt().relationship_count(), 3);
    assert!(first.receipt().receipt_digest().starts_with("blake3-256:"));
}

#[test]
fn topology_rebuild_retracts_relationships_whose_parser_premise_was_deleted() {
    let builder = builder(limits());
    let before = builder
        .build(
            "topology-generation-before",
            &[
                edge("declares-a-b", "DECLARES", "a", "b"),
                edge("calls-b-c", "CALLS", "b", "c"),
            ],
        )
        .expect("complete topology closure before deletion");
    let after = builder
        .build(
            "topology-generation-after",
            &[edge("declares-a-b", "DECLARES", "a", "b")],
        )
        .expect("complete topology closure after deletion");

    assert!(
        before
            .relationships()
            .iter()
            .any(|relationship| relationship.from() == "a" && relationship.to() == "c")
    );
    assert!(
        after
            .relationships()
            .iter()
            .all(|relationship| !(relationship.from() == "a" && relationship.to() == "c"))
    );
    assert_ne!(
        before.receipt().receipt_digest(),
        after.receipt().receipt_digest()
    );
}

#[test]
fn topology_closure_rejects_output_truncation_instead_of_claiming_fixed_point() {
    let builder = builder(ProjectTopologyClosureLimits::new(16, 64, 1).expect("bounded limits"));
    let error = builder
        .build(
            "topology-generation-truncated",
            &[
                edge("declares-a-b", "DECLARES", "a", "b"),
                edge("calls-b-c", "CALLS", "b", "c"),
            ],
        )
        .expect_err("truncated closure cannot be admitted");

    assert_eq!(error.reason_kind(), "topology-closure-incomplete");
}

#[test]
fn topology_receipt_identity_commits_parser_relation_kinds() {
    let builder = builder(limits());
    let calls = builder
        .build(
            "topology-generation-1",
            &[edge("edge-a-b", "CALLS", "a", "b")],
        )
        .expect("CALLS closure");
    let explains = builder
        .build(
            "topology-generation-1",
            &[edge("edge-a-b", "EXPLAINS", "a", "b")],
        )
        .expect("EXPLAINS closure");

    assert_eq!(calls.relationships(), explains.relationships());
    assert_ne!(
        calls.receipt().receipt_digest(),
        explains.receipt().receipt_digest(),
        "equal reachability cannot erase the parser-owned relation kind",
    );
}

#[test]
fn topology_rejects_noncanonical_parser_relation_vocabulary() {
    let error = ProjectTopologyDirectEdge::new("edge-a-b", "calls", "a", "b")
        .expect_err("noncanonical relation cannot enter the source-fact boundary");
    assert_eq!(error.reason_kind(), "topology-direct-edge-relation-invalid");
}

#[test]
fn topology_closure_publishes_relation_sensitive_ascent_conclusion() {
    let closure = builder(limits())
        .build(
            "topology-generation-domain-rule",
            &[
                edge("w1", "CALLS", "refresh", "publish"),
                edge("w2", "READS_CONFIG", "publish", "hook"),
                edge("w3", "COVERS", "test", "publish"),
            ],
        )
        .expect("complete topology closure");

    let dependency = closure
        .relationships()
        .iter()
        .find(|relationship| relationship.relation() == "DEPENDS_ON_CONFIG")
        .expect("relation-sensitive Ascent join is retained");
    assert_eq!(dependency.from(), "refresh");
    assert_eq!(dependency.to(), "hook");
    assert_eq!(dependency.premise_edge_ids(), ["w1", "w2"]);
    assert_eq!(
        dependency.rule_id(),
        "mrr.topology.domain.depends-on-config.v1"
    );
    assert!(closure.relationships().iter().all(|relationship| {
        relationship.relation() != "DEPENDS_ON_CONFIG"
            || !relationship.premise_edge_ids().contains(&"w3".to_owned())
    }));
}

#[test]
fn structural_containment_is_receipted_without_quadratic_reachability() {
    let builder = builder(ProjectTopologyClosureLimits::new(16, 1, 64).expect("bounded limits"));
    let containment = (0..16)
        .map(|index| {
            edge(
                &format!("contains-{index}"),
                "CONTAINS",
                "owner",
                &format!("item-{index}"),
            )
        })
        .collect::<Vec<_>>();
    let closure = builder
        .build("topology-generation-containment", &containment)
        .expect("structural containment does not enter generic reachability");

    assert!(closure.relationships().is_empty());
    assert_eq!(closure.receipt().input_edge_count(), 16);
    assert_eq!(closure.receipt().relationship_count(), 0);
}
