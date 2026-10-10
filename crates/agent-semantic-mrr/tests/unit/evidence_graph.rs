// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use std::collections::BTreeMap;

use agent_semantic_mrr::{
    EVIDENCE_GRAPH_PROGRAM_ID, EvidenceGraphBuildInput, EvidenceGraphEdgeKind, EvidenceGraphGap,
    EvidenceGraphLimits, EvidenceGraphNode, EvidenceGraphNodeKind, EvidenceGraphNodeStatus,
    EvidenceGraphProducer, EvidenceGraphProject, EvidenceGraphSourceFact, derive_evidence_graph,
};

fn node(id: &str, kind: EvidenceGraphNodeKind) -> EvidenceGraphNode {
    EvidenceGraphNode {
        node_id: id.to_owned(),
        kind,
        label: id.to_owned(),
        owner_path: (kind == EvidenceGraphNodeKind::Owner).then(|| "src/lib.rs".to_owned()),
        candidate_id: None,
        receipt_id: None,
        snapshot_id: None,
        readiness_id: None,
        proof_id: None,
        packet_id: None,
        waiver_id: None,
        action_id: None,
        status: None,
        summary: None,
        location: None,
        fields: BTreeMap::new(),
    }
}

fn fact(id: &str, relation: &str, from: &str, to: &str) -> EvidenceGraphSourceFact {
    EvidenceGraphSourceFact {
        fact_id: id.to_owned(),
        relation: relation.to_owned(),
        from_node_id: from.to_owned(),
        to_node_id: to.to_owned(),
    }
}

fn input() -> EvidenceGraphBuildInput {
    let mut waiver = node("waiver:temporary", EvidenceGraphNodeKind::Waiver);
    waiver.status = Some(EvidenceGraphNodeStatus::Stale);
    EvidenceGraphBuildInput {
        graph_id: "runtime.review.graph".to_owned(),
        source_generation_digest: format!("blake3-256:{}", "a".repeat(64)),
        producer: EvidenceGraphProducer {
            language_id: "runtime".to_owned(),
            provider_id: "asp-runtime-server".to_owned(),
            namespace: "agent.semantic-protocols.runtime".to_owned(),
        },
        project: EvidenceGraphProject {
            root: ".".to_owned(),
            package: None,
            fields: BTreeMap::new(),
        },
        nodes: vec![
            node("owner:src.lib.rs", EvidenceGraphNodeKind::Owner),
            node(
                "invariant:public-contract",
                EvidenceGraphNodeKind::InvariantCandidate,
            ),
            node(
                "receipt:contract-test",
                EvidenceGraphNodeKind::VerificationReceipt,
            ),
            node("packet:review", EvidenceGraphNodeKind::ReviewPacket),
            waiver,
        ],
        source_facts: vec![
            fact(
                "fact:declared-by",
                "DECLARED_BY",
                "invariant:public-contract",
                "owner:src.lib.rs",
            ),
            fact(
                "fact:packet-source",
                "DERIVED_FROM",
                "invariant:public-contract",
                "packet:review",
            ),
            fact(
                "fact:verified",
                "VERIFIED_BY",
                "invariant:public-contract",
                "receipt:contract-test",
            ),
            fact(
                "fact:waived",
                "WAIVED_BY",
                "invariant:public-contract",
                "waiver:temporary",
            ),
        ],
        gaps: vec![EvidenceGraphGap {
            gap_id: "gap:waiver-renewal".to_owned(),
            owner_path: Some("src/lib.rs".to_owned()),
            summary: "waiver is stale".to_owned(),
            severity: Some("warning".to_owned()),
            fields: BTreeMap::new(),
        }],
        limits: EvidenceGraphLimits {
            max_nodes: 32,
            max_source_facts: 32,
            max_derived_edges: 32,
        },
    }
}

#[test]
fn gql_plans_drive_ascent_edges_with_replayable_attribution() {
    let output = derive_evidence_graph(input()).expect("central graph derivation");
    let graph = output.graph();
    assert_eq!(graph.summary.nodes, 5);
    assert_eq!(graph.summary.edges, 4);
    assert_eq!(graph.summary.owners, 1);
    assert_eq!(graph.summary.claims, 1);
    assert_eq!(graph.summary.stale_items, 1);
    assert_eq!(graph.summary.gaps, 1);
    assert!(graph.edges.iter().any(|edge| {
        edge.kind == EvidenceGraphEdgeKind::VerifiedBy
            && edge.fields["derivationRuleId"]
                == format!("{EVIDENCE_GRAPH_PROGRAM_ID}.invariant-verified-by-receipt")
            && edge.fields["premiseFactIds"] == serde_json::json!(["fact:verified"])
            && edge.fields["gqlPlanDigest"]
                .as_str()
                .is_some_and(|digest| digest.starts_with("blake3-256:"))
    }));
    assert!(output.receipt().complete);
    assert_eq!(output.receipt().derived_edge_count, graph.edges.len());
    assert_eq!(graph.fields["graphDigest"], output.receipt().graph_digest);
}

#[test]
fn output_is_deterministic_across_input_order() {
    let forward = derive_evidence_graph(input()).unwrap();
    let mut reversed = input();
    reversed.nodes.reverse();
    reversed.source_facts.reverse();
    let reversed = derive_evidence_graph(reversed).unwrap();
    assert_eq!(forward, reversed);
}

#[test]
fn unknown_relation_is_not_promoted_to_a_graph_edge() {
    let mut input = input();
    input.source_facts.push(fact(
        "fact:provider-private",
        "PROVIDER_PRIVATE",
        "invariant:public-contract",
        "packet:review",
    ));
    let output = derive_evidence_graph(input).unwrap();
    assert_eq!(output.graph().edges.len(), 4);
}

#[test]
fn missing_fact_endpoint_fails_closed() {
    let mut input = input();
    input.source_facts[0].to_node_id = "owner:missing".to_owned();
    let error = derive_evidence_graph(input).unwrap_err().to_string();
    assert!(error.contains("unknown endpoint"), "{error}");
}

#[test]
fn source_generation_digest_must_match_the_wire_schema() {
    let mut input = input();
    input.source_generation_digest = format!("blake3-256:{}", "A".repeat(64));
    let error = derive_evidence_graph(input).unwrap_err().to_string();
    assert!(error.contains("generation metadata is invalid"), "{error}");
}
