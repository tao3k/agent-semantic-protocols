// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use std::collections::BTreeMap;
use std::sync::Arc;

use agent_semantic_topology::{
    ProjectTopologyClosureLimits, ProjectTopologyDirectEdge, ProjectTopologyExpectedRelation,
    ProjectTopologyGenerationBuilder, ProjectTopologyGenerationIdentity,
    ProjectTopologyInferenceProgram, ProjectTopologyManifest, ProjectTopologyRelationCoverage,
    ProjectTopologySourceNode, ProjectTopologySourceSegment,
};

fn digest(byte: char) -> String {
    format!("blake3-256:{}", byte.to_string().repeat(64))
}

fn manifest() -> ProjectTopologyManifest {
    ProjectTopologyManifest::parse_org(include_str!(
        "../../../../org/templates/project.workspace-manifest.v1.org"
    ))
    .expect("Project Topology manifest")
}

async fn candidate() -> agent_semantic_topology::ProjectTopologyGenerationCandidate {
    let rust = ProjectTopologySourceSegment::new(
        "src/registry.rs",
        digest('1'),
        vec![
            ProjectTopologySourceNode::new(
                "registry",
                "rust://src/registry.rs#item/struct/Registry",
            )
            .expect("Registry node"),
            ProjectTopologySourceNode::new(
                "refresh",
                "rust://src/registry.rs#item/method/refresh/scope/implementation-owner/type/Registry",
            )
            .expect("refresh node"),
        ],
        vec![
            ProjectTopologyDirectEdge::new("declares-refresh", "CALLS", "registry", "refresh")
                .expect("declares edge"),
        ],
    )
    .expect("Rust source segment");
    let org = ProjectTopologySourceSegment::new(
        "docs/publication.org",
        digest('2'),
        vec![
            ProjectTopologySourceNode::new(
                "publication",
                "org://docs/publication.org#item/heading/Publication",
            )
            .expect("Publication node"),
        ],
        vec![
            ProjectTopologyDirectEdge::new(
                "explains-refresh",
                "READS_CONFIG",
                "refresh",
                "publication",
            )
            .expect("explains edge"),
        ],
    )
    .expect("Org source segment");
    ProjectTopologyGenerationBuilder::new(
        ProjectTopologyGenerationIdentity::new(
            manifest().project_workspace().clone(),
            digest('3'),
            digest('4'),
            Arc::new(ProjectTopologyInferenceProgram::standard().expect("standard MRR program")),
            digest('6'),
            digest('7'),
            digest('8'),
        )
        .expect("generation identity"),
        ProjectTopologyClosureLimits::new(64, 256, 256).expect("closure limits"),
    )
    .with_expected_relations(vec![
        ProjectTopologyExpectedRelation::new(
            "expected-config",
            "registry",
            "READS_CONFIG",
            "publication",
            "heading",
            2,
            ProjectTopologyRelationCoverage::None,
        )
        .expect("expected relation"),
    ])
    .expect("unique expectations")
    .build_from_scratch(vec![rust, org])
    .await
    .expect("from-scratch topology candidate")
}

#[tokio::test]
async fn generation_builder_emits_cross_language_closure_and_requires_external_receipt_admission() {
    let candidate = candidate().await;
    let packet = candidate.packet();
    assert!(
        packet["edges"]
            .as_array()
            .expect("edges")
            .iter()
            .any(|edge| {
                edge["modality"] == "derived"
                    && edge["relation"] == "TOPOLOGY_REACHABLE"
                    && edge["from"] == "registry"
                    && edge["to"] == "publication"
                    && edge["segmentId"].is_null()
            })
    );
    assert_eq!(
        packet["frontiers"],
        serde_json::json!([{
            "anchor": "registry",
            "target": "publication",
            "relation": "READS_CONFIG",
            "targetKind": "heading",
            "depth": 2,
            "state": "unknown",
            "reason": "binding-not-established"
        }])
    );
    assert!(
        packet["edges"]
            .as_array()
            .expect("edges")
            .iter()
            .any(|edge| {
                edge["modality"] == "derived"
                    && edge["relation"] == "DEPENDS_ON_CONFIG"
                    && edge["from"] == "registry"
                    && edge["to"] == "publication"
                    && edge["witnesses"]
                        == serde_json::json!(["declares-refresh", "explains-refresh"])
            })
    );

    let error = candidate
        .clone()
        .admit(&BTreeMap::new())
        .expect_err("candidate cannot independently admit its own rebuild receipt");
    assert_eq!(error.reason_kind(), "topology-rebuild-receipt-unadmitted");

    let mut admitted = BTreeMap::new();
    admitted.insert(
        candidate.rebuild_receipt_id().to_owned(),
        candidate.rebuild_receipt().clone(),
    );
    let library = candidate
        .admit(&admitted)
        .expect("external full receipt admits the generated library");
    assert_eq!(library.segment_count(), 2);
    assert_eq!(library.node_count(), 3);
}

#[tokio::test]
async fn generation_builder_is_content_deterministic() {
    let first = candidate().await;
    let second = candidate().await;
    assert_eq!(first.packet(), second.packet());
    assert_eq!(first.rebuild_receipt(), second.rebuild_receipt());
}

#[tokio::test]
async fn generation_identity_is_derived_from_the_executed_mrr_bundle() {
    let candidate = candidate().await;
    let executed = ProjectTopologyInferenceProgram::standard()
        .expect("standard MRR program")
        .digest()
        .to_owned();
    assert_eq!(
        candidate.packet()["identities"]["inferenceProgramDigest"],
        executed
    );
    assert_eq!(
        candidate.rebuild_receipt()["inferenceProgramDigest"],
        executed
    );
}

#[test]
fn topology_builder_keeps_at_least_four_cpu_build_lanes() {
    let builder = ProjectTopologyGenerationBuilder::new(
        ProjectTopologyGenerationIdentity::new(
            manifest().project_workspace().clone(),
            digest('3'),
            digest('4'),
            Arc::new(ProjectTopologyInferenceProgram::standard().expect("standard MRR program")),
            digest('6'),
            digest('7'),
            digest('8'),
        )
        .expect("generation identity"),
        ProjectTopologyClosureLimits::new(64, 256, 256).expect("closure limits"),
    );
    assert!(builder.blocking_parallelism() >= 4);
}

#[tokio::test]
async fn incremental_generation_retracts_removed_segment_and_transitive_derivations() {
    let predecessor = candidate().await;
    let mut admitted = BTreeMap::new();
    admitted.insert(
        predecessor.rebuild_receipt_id().to_owned(),
        predecessor.rebuild_receipt().clone(),
    );
    let predecessor = Arc::new(
        predecessor
            .admit(&admitted)
            .expect("predecessor topology library"),
    );
    let predecessor_generation = predecessor.generation_digest().to_owned();
    let successor = ProjectTopologyGenerationBuilder::new(
        ProjectTopologyGenerationIdentity::new(
            manifest().project_workspace().clone(),
            digest('9'),
            digest('4'),
            Arc::new(ProjectTopologyInferenceProgram::standard().expect("standard MRR program")),
            digest('6'),
            digest('7'),
            digest('8'),
        )
        .expect("successor generation identity"),
        ProjectTopologyClosureLimits::new(64, 256, 256).expect("closure limits"),
    )
    .build_incremental(
        Arc::clone(&predecessor),
        Vec::new(),
        vec!["docs/publication.org".to_owned()],
    )
    .await
    .expect("incremental topology candidate");

    assert_eq!(
        successor.packet()["generation"]["parentGenerationDigest"],
        predecessor_generation
    );
    assert_eq!(
        successor.packet()["generation"]["removedNodeIds"],
        serde_json::json!(["publication"])
    );
    let removed_edges = successor.packet()["generation"]["removedEdgeIds"]
        .as_array()
        .expect("removed edge identities");
    assert_eq!(removed_edges.len(), 3);
    assert!(removed_edges.iter().any(|edge| edge == "explains-refresh"));
    assert!(
        removed_edges
            .iter()
            .filter_map(serde_json::Value::as_str)
            .any(|edge| edge.starts_with("derived-"))
    );
    assert!(
        successor.packet()["edges"]
            .as_array()
            .expect("successor edges")
            .iter()
            .all(|edge| edge["to"] != "publication")
    );
    let mut successor_receipts = BTreeMap::new();
    successor_receipts.insert(
        successor.rebuild_receipt_id().to_owned(),
        successor.rebuild_receipt().clone(),
    );
    let admitted_successor = successor
        .admit(&successor_receipts)
        .expect("incremental candidate remains a fully admitted library");
    assert_eq!(admitted_successor.segment_count(), 1);
    assert_eq!(admitted_successor.node_count(), 2);
}
