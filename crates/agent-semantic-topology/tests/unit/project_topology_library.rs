// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use agent_semantic_topology::{ProjectTopologyLibrary, ProjectTopologyManifest};
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

fn manifest() -> ProjectTopologyManifest {
    ProjectTopologyManifest::parse_org(include_str!(
        "../../../../org/templates/project.workspace-manifest.v1.org"
    ))
    .expect("Project Topology manifest")
}

fn admit_library(
    packet: serde_json::Value,
    receipts: &BTreeMap<String, serde_json::Value>,
) -> Result<ProjectTopologyLibrary, agent_semantic_topology::ProjectTopologyLibraryError> {
    let manifest = manifest();
    ProjectTopologyLibrary::admit_with_receipts(packet, manifest.project_workspace(), receipts)
}

#[test]
fn project_topology_library_admits_a_complete_cross_consumer_generation() {
    let library = admit_library(valid_library(), &admitted_receipts())
        .expect("complete topology generation must be admitted");

    assert_eq!(library.node_count(), 4);
    assert_eq!(library.segment_count(), 2);
    assert!(library.supports_consumer("framework-calibration"));
    assert_eq!(
        library.project_workspace_identity(),
        "git+https://github.com/tao3k/agent-semantic-protocols.git#workspace/root"
    );
    assert_eq!(library.workspace_root_path(), ".");
}

#[test]
fn query_relationship_is_the_least_incident_admitted_edge_identity() {
    let library = admit_library(valid_library(), &admitted_receipts()).unwrap();
    let relationship = library
        .canonical_relationship_for_selector(
            "rust://src/registry.rs#item/method/refresh_registry/scope/implementation-owner/type/Registry",
        )
        .expect("selector has two incident topology edges");
    assert_eq!(relationship.edge_id(), "declares-refresh");
    assert_eq!(relationship.from_node(), "Registry");
    assert_eq!(relationship.relation(), "DECLARES");
    assert_eq!(relationship.to_node(), "refresh_registry");
}

#[test]
fn query_relationship_rejects_a_selector_absent_from_topology() {
    let library = admit_library(valid_library(), &admitted_receipts()).unwrap();
    let error = library
        .canonical_relationship_for_selector("rust://src/missing.rs#item/function/missing")
        .expect_err("Query cannot synthesize a placeholder topology relationship");
    assert_eq!(error.reason_kind(), "topology-query-selector-node-missing");
}

#[test]
fn project_topology_library_rejects_runtime_or_worktree_identity_as_project_workspace() {
    for identity in [
        "workspace-23cc5ba784c605ae",
        "git+https://github.com/tao3k/agent-semantic-protocols.git#worktree/feature",
    ] {
        let mut packet = valid_library();
        packet["projectWorkspace"]["projectWorkspaceIdentity"] = serde_json::json!(identity);
        let error = admit_library(packet, &admitted_receipts())
            .expect_err("Runtime and worktree identifiers cannot impersonate a workspace");
        assert_eq!(
            error.reason_kind(),
            "topology-project-workspace-identity-invalid"
        );
    }
}

#[test]
fn project_topology_library_rejects_another_valid_workspace_not_declared_by_manifest() {
    let mut packet = valid_library();
    packet["projectWorkspace"]["projectWorkspaceIdentity"] =
        serde_json::json!("git+https://github.com/tao3k/another-project.git#workspace/root");
    let error = admit_library(packet, &admitted_receipts())
        .expect_err("a syntax-valid workspace cannot bypass the manifest authority");
    assert_eq!(error.reason_kind(), "topology-project-workspace-mismatch");
}

#[test]
fn project_topology_library_rejects_absolute_or_traversing_workspace_root() {
    for root in ["/tmp/repo", "../repo", "src/../docs"] {
        let mut packet = valid_library();
        packet["projectWorkspace"]["workspaceRootPath"] = serde_json::json!(root);
        let error = admit_library(packet, &admitted_receipts())
            .expect_err("workspace roots must be normalized repository-relative paths");
        assert_eq!(error.reason_kind(), "topology-workspace-root-path-invalid");
    }
}

#[test]
fn project_topology_library_rejects_local_locator_claiming_cross_machine_portability() {
    let mut packet = valid_library();
    packet["projectWorkspace"]["projectWorkspaceIdentity"] =
        serde_json::json!("git+file:///tmp/agent-semantic-protocols.git#workspace/root");
    let error = admit_library(packet, &admitted_receipts())
        .expect_err("a local file locator cannot claim cross-machine portability");
    assert_eq!(
        error.reason_kind(),
        "topology-project-workspace-portability-mismatch"
    );
}

#[test]
fn project_topology_library_rejects_non_equivalent_incremental_publication() {
    let mut packet = valid_library();
    packet["generation"]["fromScratchEquivalentDigest"] = serde_json::json!(
        "blake3-256:abababababababababababababababababababababababababababababababab"
    );

    let error = admit_library(packet, &admitted_receipts())
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

    let error = admit_library(packet, &admitted_receipts())
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

    let error = admit_library(packet, &admitted_receipts())
        .expect_err("a removed node cannot survive in the complete generation");
    assert_eq!(error.reason_kind(), "topology-removed-node-still-active");
}

#[test]
fn project_topology_library_rejects_dangling_edges() {
    let mut packet = valid_library();
    packet["edges"][0]["to"] = serde_json::json!("missing");

    let error = admit_library(packet, &admitted_receipts())
        .expect_err("every topology edge endpoint must exist");
    assert_eq!(error.reason_kind(), "topology-dangling-edge");
}

#[test]
fn project_topology_library_rejects_semantic_annotation_identity_drift() {
    let mut packet = valid_library();
    packet["nodes"][3]["annotation"]["bindingDigest"] = serde_json::json!(
        "blake3-256:abababababababababababababababababababababababababababababababab"
    );

    let error = admit_library(packet, &admitted_receipts())
        .expect_err("semantic annotations must bind the admitted semantic topology");
    assert_eq!(error.reason_kind(), "topology-annotation-binding-mismatch");
}

#[test]
fn project_topology_library_rejects_incomplete_derived_closure_inventory() {
    let mut packet = valid_library();
    packet["edges"].as_array_mut().expect("edges").push(serde_json::json!({
        "id": "derived-publication-path",
        "segmentId": null,
        "from": "refresh",
        "to": "publication",
        "relation": "DOCUMENTED_PATH",
        "modality": "derived",
        "bindingDigest": "blake3-256:8888888888888888888888888888888888888888888888888888888888888888",
        "witnesses": ["declares-refresh", "explains-refresh"],
        "proofRef": "proof-path-1"
    }));

    let error = admit_library(packet, &admitted_receipts())
        .expect_err("stable closure must inventory every derived edge");
    assert_eq!(error.reason_kind(), "topology-derived-closure-mismatch");
}

#[test]
fn project_topology_library_admits_generation_owned_cross_segment_derivation() {
    let mut packet = valid_library();
    packet["edges"].as_array_mut().expect("edges").push(serde_json::json!({
        "id": "derived-publication-path",
        "segmentId": null,
        "from": "registry",
        "to": "publication",
        "relation": "TOPOLOGY_REACHABLE",
        "modality": "derived",
        "bindingDigest": "blake3-256:8888888888888888888888888888888888888888888888888888888888888888",
        "witnesses": ["declares-refresh", "explains-refresh"],
        "proofRef": "proof-path-1"
    }));
    packet["closure"]["derivedEdgeIds"] = serde_json::json!(["derived-publication-path"]);
    packet["closure"]["proofDependencies"] = serde_json::json!([{
        "derivedEdgeId": "derived-publication-path",
        "premiseEdgeIds": ["declares-refresh", "explains-refresh"]
    }]);

    admit_library(packet, &admitted_receipts())
        .expect("a derived cross-segment relationship belongs to the topology generation");
}

#[test]
fn project_topology_library_rejects_derived_edge_claiming_source_segment_ownership() {
    let mut packet = valid_library();
    packet["edges"].as_array_mut().expect("edges").push(serde_json::json!({
        "id": "derived-publication-path",
        "segmentId": "segment-org",
        "from": "registry",
        "to": "publication",
        "relation": "TOPOLOGY_REACHABLE",
        "modality": "derived",
        "bindingDigest": "blake3-256:8888888888888888888888888888888888888888888888888888888888888888",
        "witnesses": ["declares-refresh", "explains-refresh"],
        "proofRef": "proof-path-1"
    }));
    packet["closure"]["derivedEdgeIds"] = serde_json::json!(["derived-publication-path"]);
    packet["closure"]["proofDependencies"] = serde_json::json!([{
        "derivedEdgeId": "derived-publication-path",
        "premiseEdgeIds": ["declares-refresh", "explains-refresh"]
    }]);

    let error = admit_library(packet, &admitted_receipts())
        .expect_err("inference output cannot impersonate source-segment evidence");
    assert_eq!(
        error.reason_kind(),
        "topology-inferred-edge-source-segment-forbidden"
    );
}

#[test]
fn project_topology_library_requires_external_rebuild_receipt_admission() {
    let admitted = admitted_receipts();
    admit_library(valid_library(), &admitted)
        .expect("the exact externally admitted rebuild receipt is accepted");

    let error = admit_library(valid_library(), &BTreeMap::new())
        .expect_err("an embedded receipt cannot authorize itself");
    assert_eq!(error.reason_kind(), "topology-rebuild-receipt-unadmitted");

    let mut forged = valid_library();
    forged["fromScratchRebuildReceipt"]["recomputedLibraryDigest"] = serde_json::json!(
        "blake3-256:abababababababababababababababababababababababababababababababab"
    );
    let error = admit_library(forged, &admitted)
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
        let error = admit_library(packet, &admitted)
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
    admit_library(packet.clone(), &admitted)
        .expect("the exact externally admitted annotation receipt is accepted");

    packet["semanticAdmissionReceipts"][0]["annotationNodeId"] = serde_json::json!("publication");
    let error = admit_library(packet, &admitted)
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
        "segmentId": null,
        "from": "publication",
        "to": "refresh",
        "relation": "DOCUMENTED_PATH",
        "modality": "derived",
        "bindingDigest": "blake3-256:8888888888888888888888888888888888888888888888888888888888888888",
        "witnesses": ["explains-refresh"],
        "proofRef": "proof-path-1"
    }));
    packet["closure"]["derivedEdgeIds"] = serde_json::json!(["derived-publication-path"]);
    packet["closure"]["proofDependencies"] = serde_json::json!([{
        "derivedEdgeId": "derived-publication-path",
        "premiseEdgeIds": ["declares-refresh"]
    }]);

    let error = admit_library(packet, &admitted_receipts())
        .expect_err("deleting a premise must invalidate every transitive derived descendant");
    assert_eq!(
        error.reason_kind(),
        "topology-derived-dependency-invalidated"
    );
}
