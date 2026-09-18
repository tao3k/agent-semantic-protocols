// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use std::collections::BTreeMap;

use agent_semantic_mrr::{
    EvidenceGraphLimits, EvidenceGraphNode, EvidenceGraphNodeKind, EvidenceGraphProject,
    EvidenceGraphSourceFact,
};

use super::{RuntimeEvidenceGraphBuildInput, build_runtime_evidence_graph};

fn node(id: &str, kind: EvidenceGraphNodeKind) -> EvidenceGraphNode {
    EvidenceGraphNode {
        node_id: id.to_owned(),
        kind,
        label: id.to_owned(),
        owner_path: None,
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

#[test]
fn runtime_is_the_fixed_graph_producer_and_mrr_is_the_derivation_authority() {
    let output = build_runtime_evidence_graph(RuntimeEvidenceGraphBuildInput {
        graph_id: "runtime.evidence.graph".to_owned(),
        source_generation_digest: format!("blake3-256:{}", "a".repeat(64)),
        project: EvidenceGraphProject {
            root: ".".to_owned(),
            package: None,
            fields: BTreeMap::new(),
        },
        nodes: vec![
            node(
                "invariant:central-authority",
                EvidenceGraphNodeKind::InvariantCandidate,
            ),
            node(
                "receipt:runtime-test",
                EvidenceGraphNodeKind::VerificationReceipt,
            ),
        ],
        source_facts: vec![EvidenceGraphSourceFact {
            fact_id: "fact:verified-by".to_owned(),
            relation: "VERIFIED_BY".to_owned(),
            from_node_id: "invariant:central-authority".to_owned(),
            to_node_id: "receipt:runtime-test".to_owned(),
        }],
        gaps: Vec::new(),
        limits: EvidenceGraphLimits {
            max_nodes: 8,
            max_source_facts: 8,
            max_derived_edges: 8,
        },
    })
    .expect("Runtime EvidenceGraph build");

    assert_eq!(output.graph().producer.language_id, "runtime");
    assert_eq!(output.graph().producer.provider_id, "asp-runtime-server");
    assert_eq!(output.graph().edges.len(), 1);
    assert!(
        output.graph().edges[0].fields["derivationRuleId"]
            .as_str()
            .is_some_and(|rule| rule.starts_with("mrr.evidence-graph.runtime."))
    );
    assert_eq!(output.receipt().input_fact_count, 1);
    assert!(output.receipt().complete);
}
