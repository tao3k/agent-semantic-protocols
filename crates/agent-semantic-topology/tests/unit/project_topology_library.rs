use agent_semantic_topology::ProjectTopologyLibrary;
use std::collections::BTreeMap;

fn valid_library() -> serde_json::Value {
    serde_json::from_str(include_str!(
        "../../../../schemas/fixtures/project-topology-library/valid-polyglot.v1.json"
    ))
    .expect("valid Project Topology fixture JSON")
}

fn admitted_receipts() -> BTreeMap<String, serde_json::Value> {
    let packet = valid_library();
    BTreeMap::from([(
        "topology-rebuild-1".to_owned(),
        packet["fromScratchRebuildReceipt"].clone(),
    )])
}

#[test]
fn project_topology_library_admits_a_complete_cross_consumer_generation() {
    let library =
        ProjectTopologyLibrary::admit_with_receipts(valid_library(), &admitted_receipts())
            .expect("complete topology generation must be admitted");

    assert_eq!(library.node_count(), 4);
    assert_eq!(library.segment_count(), 2);
    assert!(library.supports_consumer("framework-calibration"));
}

#[test]
fn project_topology_library_rejects_non_equivalent_incremental_publication() {
    let mut packet = valid_library();
    packet["generation"]["fromScratchEquivalentDigest"] = serde_json::json!(
        "blake3-256:abababababababababababababababababababababababababababababababab"
    );

    let error = ProjectTopologyLibrary::admit_with_receipts(packet, &admitted_receipts())
        .expect_err("incremental publication must equal a from-scratch rebuild");
    assert_eq!(
        error.reason_kind(),
        "topology-from-scratch-equivalence-mismatch"
    );
}

#[test]
fn project_topology_library_rejects_segment_membership_drift() {
    let mut packet = valid_library();
    packet["segments"][0]["nodeIds"] = serde_json::json!(["registry"]);

    let error = ProjectTopologyLibrary::admit_with_receipts(packet, &admitted_receipts())
        .expect_err("segment membership must account for every active node exactly once");
    assert_eq!(
        error.reason_kind(),
        "topology-segment-node-membership-mismatch"
    );
}

#[test]
fn project_topology_library_rejects_live_removed_nodes() {
    let mut packet = valid_library();
    packet["generation"]["removedNodeIds"] = serde_json::json!(["refresh"]);

    let error = ProjectTopologyLibrary::admit_with_receipts(packet, &admitted_receipts())
        .expect_err("a removed node cannot survive in the complete generation");
    assert_eq!(error.reason_kind(), "topology-removed-node-still-active");
}

#[test]
fn project_topology_library_rejects_dangling_edges() {
    let mut packet = valid_library();
    packet["edges"][0]["to"] = serde_json::json!("missing");

    let error = ProjectTopologyLibrary::admit_with_receipts(packet, &admitted_receipts())
        .expect_err("every topology edge endpoint must exist");
    assert_eq!(error.reason_kind(), "topology-dangling-edge");
}

#[test]
fn project_topology_library_rejects_semantic_annotation_identity_drift() {
    let mut packet = valid_library();
    packet["nodes"][3]["annotation"]["bindingDigest"] = serde_json::json!(
        "blake3-256:abababababababababababababababababababababababababababababababab"
    );

    let error = ProjectTopologyLibrary::admit_with_receipts(packet, &admitted_receipts())
        .expect_err("semantic annotations must bind the admitted semantic topology");
    assert_eq!(error.reason_kind(), "topology-annotation-binding-mismatch");
}

#[test]
fn project_topology_library_rejects_incomplete_derived_closure_inventory() {
    let mut packet = valid_library();
    packet["edges"].as_array_mut().expect("edges").push(serde_json::json!({
        "id": "derived-publication-path",
        "segmentId": "segment-org",
        "from": "refresh",
        "to": "publication",
        "relation": "DOCUMENTED_PATH",
        "modality": "derived",
        "bindingDigest": "blake3-256:8888888888888888888888888888888888888888888888888888888888888888",
        "witnesses": ["declares-refresh", "explains-refresh"],
        "proofRef": "proof-path-1"
    }));
    packet["segments"][1]["edgeIds"] =
        serde_json::json!(["derived-publication-path", "explains-refresh"]);

    let error = ProjectTopologyLibrary::admit_with_receipts(packet, &admitted_receipts())
        .expect_err("stable closure must inventory every derived edge");
    assert_eq!(error.reason_kind(), "topology-derived-closure-mismatch");
}

#[test]
fn project_topology_library_requires_external_rebuild_receipt_admission() {
    let admitted = admitted_receipts();
    ProjectTopologyLibrary::admit_with_receipts(valid_library(), &admitted)
        .expect("the exact externally admitted rebuild receipt is accepted");

    let error = ProjectTopologyLibrary::admit_with_receipts(valid_library(), &BTreeMap::new())
        .expect_err("an embedded receipt cannot authorize itself");
    assert_eq!(error.reason_kind(), "topology-rebuild-receipt-unadmitted");

    let mut forged = valid_library();
    forged["fromScratchRebuildReceipt"]["recomputedLibraryDigest"] = serde_json::json!(
        "blake3-256:abababababababababababababababababababababababababababababababab"
    );
    let error = ProjectTopologyLibrary::admit_with_receipts(forged, &admitted)
        .expect_err("reusing an admitted receipt ID cannot authorize different receipt content");
    assert_eq!(error.reason_kind(), "topology-rebuild-receipt-unadmitted");
}

#[test]
fn project_topology_library_rejects_cross_source_or_program_rebuild_replay() {
    for (field, digest) in [
        (
            "sourceGenerationDigest",
            "blake3-256:dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd",
        ),
        (
            "inferenceProgramDigest",
            "blake3-256:eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee",
        ),
    ] {
        let mut packet = valid_library();
        packet["fromScratchRebuildReceipt"][field] = serde_json::json!(digest);
        let mut admitted = BTreeMap::new();
        admitted.insert(
            "topology-rebuild-1".to_owned(),
            packet["fromScratchRebuildReceipt"].clone(),
        );
        let error = ProjectTopologyLibrary::admit_with_receipts(packet, &admitted)
            .expect_err("a rebuild receipt from another source or program must not replay");
        assert_eq!(error.reason_kind(), "topology-rebuild-receipt-mismatch");
    }
}

#[test]
fn project_topology_library_requires_exact_semantic_annotation_receipt_binding() {
    let mut packet = valid_library();
    packet["nodes"][3]["annotation"]["state"] = serde_json::json!("accepted");
    packet["nodes"][3]["annotation"]["admissionReceiptRef"] =
        serde_json::json!("semantic-receipt-1");
    packet["semanticAdmissionReceipts"] = serde_json::json!([{
        "id": "semantic-receipt-1",
        "annotationNodeId": "meaning",
        "semanticTopologyDigest": "blake3-256:7777777777777777777777777777777777777777777777777777777777777777",
        "authority": "human-admission",
        "state": "admitted"
    }]);

    let mut admitted = admitted_receipts();
    admitted.insert(
        "semantic-receipt-1".to_owned(),
        packet["semanticAdmissionReceipts"][0].clone(),
    );
    ProjectTopologyLibrary::admit_with_receipts(packet.clone(), &admitted)
        .expect("the exact externally admitted annotation receipt is accepted");

    packet["semanticAdmissionReceipts"][0]["annotationNodeId"] = serde_json::json!("publication");
    let error = ProjectTopologyLibrary::admit_with_receipts(packet, &admitted)
        .expect_err("a receipt for another topology node cannot admit the annotation");
    assert_eq!(
        error.reason_kind(),
        "topology-annotation-admission-receipt-mismatch"
    );
}

#[test]
fn project_topology_library_invalidates_transitive_derived_edges_after_premise_deletion() {
    let mut packet = valid_library();
    packet["edges"].as_array_mut().expect("edges").remove(0);
    packet["segments"][0]["edgeIds"] = serde_json::json!([]);
    packet["generation"]["removedEdgeIds"] = serde_json::json!(["declares-refresh"]);
    packet["edges"].as_array_mut().expect("edges").push(serde_json::json!({
        "id": "derived-publication-path",
        "segmentId": "segment-org",
        "from": "publication",
        "to": "refresh",
        "relation": "DOCUMENTED_PATH",
        "modality": "derived",
        "bindingDigest": "blake3-256:8888888888888888888888888888888888888888888888888888888888888888",
        "witnesses": ["explains-refresh"],
        "proofRef": "proof-path-1"
    }));
    packet["segments"][1]["edgeIds"] =
        serde_json::json!(["derived-publication-path", "explains-refresh"]);
    packet["closure"]["derivedEdgeIds"] = serde_json::json!(["derived-publication-path"]);
    packet["closure"]["proofDependencies"] = serde_json::json!([{
        "derivedEdgeId": "derived-publication-path",
        "premiseEdgeIds": ["declares-refresh"]
    }]);

    let error = ProjectTopologyLibrary::admit_with_receipts(packet, &admitted_receipts())
        .expect_err("deleting a premise must invalidate every transitive derived descendant");
    assert_eq!(
        error.reason_kind(),
        "topology-derived-dependency-invalidated"
    );
}
