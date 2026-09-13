// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use agent_semantic_search_projection::SearchTopologySettlement;
use agent_semantic_topology::{
    ProjectTopologyClosureLimits, ProjectTopologyDirectEdge, ProjectTopologyExpectedRelation,
    ProjectTopologyGenerationBuilder, ProjectTopologyGenerationIdentity,
    ProjectTopologyInferenceProgram, ProjectTopologyLibrary, ProjectTopologyManifest,
    ProjectTopologyRelationCoverage, ProjectTopologySourceNode, ProjectTopologySourceSegment,
};
use std::collections::BTreeMap;
use std::sync::Arc;

fn valid_settlement() -> serde_json::Value {
    serde_json::from_str(include_str!(
        "../../../../schemas/fixtures/search-topology-settlement/valid-derived-and-proposed.v1.json"
    ))
    .expect("valid settlement fixture JSON")
}

fn valid_library() -> ProjectTopologyLibrary {
    let packet: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../schemas/fixtures/project-topology-library/valid-polyglot.v1.json"
    ))
    .expect("valid Project Topology fixture JSON");
    let receipts = BTreeMap::from([(
        "topology-rebuild-1".to_owned(),
        packet["fromScratchRebuildReceipt"].clone(),
    )]);
    let manifest = ProjectTopologyManifest::parse_org(include_str!(
        "../../../../org/templates/project.workspace-manifest.v1.org"
    ))
    .expect("Project Topology manifest");
    ProjectTopologyLibrary::admit_with_receipts(packet, manifest.project_workspace(), &receipts)
        .expect("admitted Project Topology fixture")
}

fn digest(byte: char) -> String {
    format!("blake3-256:{}", byte.to_string().repeat(64))
}

async fn generated_v2_library_with_frontier() -> ProjectTopologyLibrary {
    let manifest = ProjectTopologyManifest::parse_org(include_str!(
        "../../../../org/templates/project.workspace-manifest.v1.org"
    ))
    .expect("Project Topology manifest");
    let source = ProjectTopologySourceSegment::new(
        "src/registry.rs",
        digest('1'),
        vec![
            ProjectTopologySourceNode::new(
                "registry",
                "rust://src/registry.rs#item/struct/Registry",
            )
            .expect("registry node"),
            ProjectTopologySourceNode::new(
                "refresh",
                "rust://src/registry.rs#item/method/refresh/scope/implementation-owner/type/Registry",
            )
            .expect("refresh node"),
            ProjectTopologySourceNode::new(
                "publication",
                "org://docs/publication.org#item/heading/Publication",
            )
            .expect("publication node"),
        ],
        vec![
            ProjectTopologyDirectEdge::new("calls-refresh", "CALLS", "registry", "refresh")
                .expect("CALLS edge"),
            ProjectTopologyDirectEdge::new(
                "covers-publication",
                "COVERS",
                "refresh",
                "publication",
            )
            .expect("COVERS edge"),
        ],
    )
    .expect("source segment");
    let candidate = ProjectTopologyGenerationBuilder::new(
        ProjectTopologyGenerationIdentity::new(
            manifest.project_workspace().clone(),
            digest('2'),
            digest('3'),
            Arc::new(ProjectTopologyInferenceProgram::standard().expect("standard MRR program")),
            digest('4'),
            digest('5'),
            digest('6'),
        )
        .expect("generation identity"),
        ProjectTopologyClosureLimits::new(64, 256, 256).expect("closure limits"),
    )
    .with_expected_relations(vec![
        ProjectTopologyExpectedRelation::new(
            "expected-publication",
            "registry",
            "COVERS",
            "publication",
            "heading",
            1,
            ProjectTopologyRelationCoverage::Partial,
        )
        .expect("expected relation"),
    ])
    .expect("unique expectation")
    .build_from_scratch(vec![source])
    .await
    .expect("generated V1 topology");
    let admitted = BTreeMap::from([(
        candidate.rebuild_receipt_id().to_owned(),
        candidate.rebuild_receipt().clone(),
    )]);
    candidate
        .admit(&admitted)
        .expect("generated V1 topology admission")
}

#[test]
fn topology_settlement_admits_the_bound_single_gql_fixture() {
    let settlement = SearchTopologySettlement::admit(valid_settlement())
        .expect("the shared settlement fixture must be admitted");

    assert_eq!(settlement.queryable_selector_count(), 2);
    assert_eq!(settlement.derived_relation_count(), 1);
}

#[test]
fn topology_settlement_rejects_authority_that_does_not_match_edge_modality() {
    let mut packet = valid_settlement();
    packet["edges"][0]["producerAuthority"] = serde_json::json!("model-proposal");

    let error = SearchTopologySettlement::admit(packet)
        .expect_err("a derived edge cannot be admitted as a model proposal");
    assert_eq!(error.reason_kind(), "edge-authority-modality-mismatch");
}

#[test]
fn topology_settlement_requires_the_exact_reusable_library_generation() {
    let mut packet = valid_settlement();
    packet["binding"]
        .as_object_mut()
        .expect("binding")
        .remove("topologyLibraryDigest");

    let error = SearchTopologySettlement::admit(packet)
        .expect_err("a Search slice cannot float free of its topology library");
    assert_eq!(error.reason_kind(), "schema-invalid");
}

#[test]
fn topology_settlement_is_jointly_admitted_with_the_exact_library() {
    SearchTopologySettlement::admit_for_library(valid_settlement(), &valid_library())
        .expect("settlement and reusable topology library share one identity tuple");
}

#[test]
fn topology_settlement_rejects_cross_library_replay() {
    let mut packet = valid_settlement();
    packet["binding"]["topologyGenerationDigest"] = serde_json::json!(
        "blake3-256:abababababababababababababababababababababababababababababababab"
    );

    let error = SearchTopologySettlement::admit_for_library(packet, &valid_library())
        .expect_err("a settlement cannot be replayed against another topology generation");
    assert_eq!(error.reason_kind(), "topology-library-binding-mismatch");
}

#[test]
fn topology_settlement_rejects_dangling_edges() {
    let mut packet = valid_settlement();
    packet["edges"][0]["to"] = serde_json::json!("missing-node");

    let error = SearchTopologySettlement::admit(packet)
        .expect_err("every edge endpoint must exist in the settled graph");
    assert_eq!(error.reason_kind(), "dangling-edge");
}

#[test]
fn topology_settlement_rejects_duplicate_selector_ownership() {
    let mut packet = valid_settlement();
    let mut duplicate = packet["nodes"][0].clone();
    duplicate["id"] = serde_json::json!("refresh_duplicate");
    packet["nodes"]
        .as_array_mut()
        .expect("nodes")
        .push(duplicate);

    let error = SearchTopologySettlement::admit(packet)
        .expect_err("one canonical selector cannot be owned by two result nodes");
    assert_eq!(error.reason_kind(), "duplicate-selector");
}

#[test]
fn topology_settlement_rejects_the_removed_materialization_set() {
    let mut packet = valid_settlement();
    packet["materializationSet"] = serde_json::json!({"selectors": []});

    let error = SearchTopologySettlement::admit(packet)
        .expect_err("V1 cannot restore the duplicate selector handoff");
    assert_eq!(error.reason_kind(), "search-materialization-set-removed");
}

#[test]
fn topology_settlement_rejects_partial_frontier_without_coverage() {
    let mut packet = valid_settlement();
    packet["frontiers"][0]
        .as_object_mut()
        .expect("frontier")
        .remove("coverageRef");

    let error = SearchTopologySettlement::admit(packet)
        .expect_err("partial coverage must remain bound to its certificate");
    assert_eq!(error.reason_kind(), "frontier-classification-mismatch");
}

#[test]
fn topology_settlement_rejects_extraneous_coverage() {
    let mut packet = valid_settlement();
    packet["coverageCertificates"]
        .as_array_mut()
        .expect("coverage certificates")
        .push(serde_json::json!({
            "id": "coverage-unused",
            "relation": "CALLS",
            "targetKind": "Method",
            "scope": "partial",
            "digest": "blake3-256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
        }));

    let error = SearchTopologySettlement::admit(packet)
        .expect_err("request projection cannot retain unrelated coverage");
    assert_eq!(error.reason_kind(), "frontier-coverage-extraneous");
}

#[test]
fn topology_settlement_rejects_budget_terminal_without_a_real_fixed_point() {
    let mut packet = valid_settlement();
    packet["inference"]["nextRelationSetDigest"] = serde_json::json!(
        "blake3-256:ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff"
    );
    let error = SearchTopologySettlement::admit(packet)
        .expect_err("candidate and next relation sets must be identical");
    assert_eq!(error.reason_kind(), "fixed-point-not-reached");
}

#[test]
fn topology_settlement_rejects_missing_derived_proof_dependencies() {
    let mut packet = valid_settlement();
    packet["edges"][0]
        .as_object_mut()
        .expect("derived edge")
        .remove("proofRef");

    let error = SearchTopologySettlement::admit(packet)
        .expect_err("every retained derived edge must retain its proof reference");
    assert_eq!(error.reason_kind(), "schema-invalid");
}

#[test]
fn topology_settlement_rejects_cross_closure_replay() {
    let mut packet = valid_settlement();
    packet["binding"]["topologyClosureDigest"] = serde_json::json!(
        "blake3-256:ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff"
    );
    packet["inference"]["candidateClosureReceiptDigest"] = serde_json::json!(
        "blake3-256:ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff"
    );

    let error = SearchTopologySettlement::admit_for_library(packet, &valid_library())
        .expect_err("a settlement cannot replace the admitted topology closure");
    assert_eq!(error.reason_kind(), "topology-library-binding-mismatch");
}

#[test]
fn topology_settlement_rejects_a_synthetic_request_local_closure_receipt() {
    let mut packet = valid_settlement();
    packet["inference"]["candidateClosureReceiptDigest"] = serde_json::json!(
        "blake3-256:ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff"
    );

    let error = SearchTopologySettlement::admit(packet)
        .expect_err("request projection must cite the admitted topology closure receipt");
    assert_eq!(error.reason_kind(), "topology-closure-receipt-mismatch");
}

#[test]
fn budget_exhaustion_remains_incomplete_without_query_materialization() {
    let mut packet = valid_settlement();
    packet["resultState"] = serde_json::json!("incomplete");
    packet["inference"]["state"] = serde_json::json!("incomplete");
    packet["inference"]["terminationKind"] = serde_json::json!("budget-exhausted");
    packet["inference"]["postRankingCertified"] = serde_json::json!(false);
    packet["inference"]["reasonKind"] = serde_json::json!("search-inference-budget-exhausted");
    packet["inference"]["nextRelationSetDigest"] = serde_json::json!(
        "blake3-256:ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff"
    );
    packet["terminal"]["state"] = serde_json::json!("incomplete");
    packet["terminal"]["reasonKind"] = serde_json::json!("search-inference-budget-exhausted");

    SearchTopologySettlement::admit(packet)
        .expect("budget exhaustion is honest and cannot authorize Query");
}

#[test]
fn topology_settlement_renders_one_compact_polyglot_gql_result() {
    let settlement = SearchTopologySettlement::admit(valid_settlement()).unwrap();
    let rendered = settlement
        .render_org_gql()
        .expect("render admitted settlement");

    assert_eq!(rendered.matches("#+begin_src gql").count(), 1);
    assert_eq!(rendered.matches("#+end_src").count(), 1);
    assert!(!rendered.contains("#+begin_src ascent"));
    assert!(rendered.contains("(rust:Language)-[:RESULTS]->["));
    assert!(rendered.contains("refresh:RustMethod"));
    assert!(rendered.contains("selector:\"rust://src/registry.rs#item/method/refresh_registry/scope/implementation-owner/type/Registry\""));
    assert!(rendered.contains(
        "projection:{rank:1,depth:0,hit:{rg:[[42,46]],tantivy:[\"artifact refresh\"],native:true}}"
    ));
    assert!(rendered.contains("(org:Language)-[:RESULTS]->["));
    assert!(rendered.contains("meaning:SemanticAnnotation"));
    assert!(rendered.contains("state:\"proposed\""));
    assert!(rendered.contains("(refresh)-[:DOCUMENTED_PATH {modality:\"derived\""));
    assert!(rendered.contains("proof:\"proof-path-42\""));
    assert!(!rendered.contains("MaterializationSet"));
}

#[test]
fn topology_settlement_groups_same_language_nodes_in_one_results_list() {
    let mut packet = valid_settlement();
    packet["nodes"]
        .as_array_mut()
        .unwrap()
        .push(serde_json::json!({
            "id": "publish",
            "language": "rust",
            "kind": "method",
            "name": "publish_artifact",
            "selector": "rust://src/registry.rs#item/method/publish_artifact/scope/implementation-owner/type/Registry"
        }));
    let rendered = SearchTopologySettlement::admit(packet)
        .unwrap()
        .render_org_gql()
        .unwrap();

    assert_eq!(rendered.matches("(rust:Language)-[:RESULTS]->[").count(), 1);
    let rust_results = rendered
        .lines()
        .find(|line| line.starts_with("(rust:Language)-[:RESULTS]->["))
        .unwrap();
    assert!(rust_results.contains("refresh:RustMethod"));
    assert!(rust_results.contains("publish:RustMethod"));
}

#[test]
fn executed_workspace_result_is_joined_to_topology_before_rendering() {
    let library = valid_library();
    let workspace_result = serde_json::json!({
        "schemaId": "agent.semantic-protocols.workspace-search-playbook-result",
        "schemaVersion": "1",
        "result": "relationship-supported",
        "evidenceItemLimit": 30,
        "evidence": [{
            "owner": "src/registry.rs",
            "item": "method/refresh_registry/scope/implementation-owner/type/Registry",
            "selector": "rust://src/registry.rs#item/method/refresh_registry/scope/implementation-owner/type/Registry",
            "matchedBy": ["rg:0", "syntax:0", "graph:0"],
            "relation": "syntax-capture:method",
            "hit": {"rg": [[42, 42]], "native": true}
        }]
    });
    let settlement = SearchTopologySettlement::from_workspace_result(
        "search-request-1",
        &workspace_result,
        &library,
    )
    .expect("Runtime Search evidence joins the attached topology");

    assert_eq!(settlement.as_json()["resultState"], "queryable");
    assert_eq!(settlement.queryable_selector_count(), 3);
    let selected_node = settlement.as_json()["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|node| node["selector"] == workspace_result["evidence"][0]["selector"])
        .unwrap();
    assert_eq!(
        selected_node["projection"]["hit"],
        workspace_result["evidence"][0]["hit"]
    );
    assert!(
        settlement
            .render_org_gql()
            .unwrap()
            .contains("(registry)-[:DECLARES")
    );
    assert!(
        settlement
            .render_org_gql()
            .unwrap()
            .contains("projection:{rank:1,depth:0,hit:{rg:[[42,42]],native:true}}")
    );
}

#[tokio::test]
async fn generated_v2_frontier_is_preserved_in_public_gql() {
    let library = generated_v2_library_with_frontier().await;
    let settlement = SearchTopologySettlement::from_workspace_result(
        "search-request-frontier",
        &serde_json::json!({
            "schemaId": "agent.semantic-protocols.workspace-search-playbook-result",
            "schemaVersion": "1",
            "result": "exact-selector-ready",
            "evidenceItemLimit": 30,
            "evidence": [{
                "owner": "src/registry.rs",
                "item": "struct/Registry",
                "selector": "rust://src/registry.rs#item/struct/Registry",
                "matchedBy": ["native-syntax:0"],
                "relation": "native-selector",
                "hit": {"native": true}
            }]
        }),
        &library,
    )
    .expect("frontier-bearing Search settlement");

    assert_eq!(settlement.as_json()["schemaVersion"], "1");
    assert_eq!(
        settlement.as_json()["frontiers"].as_array().unwrap().len(),
        1
    );
    let gql = settlement.render_org_gql().unwrap();
    assert!(gql.contains("(registry)-[:CALLS {modality:\"parser-direct\""));
    assert!(gql.contains("(refresh)-[:COVERS {modality:\"parser-direct\""));
    assert!(gql.contains(
        "(registry)-[:FRONTIER {relation:\"COVERS\",target_kind:\"heading\",depth:1,state:\"unknown\",reason:\"coverage-open\",coverage:\"coverage-expected-publication\"}]->(publication)"
    ));
}

#[test]
fn empty_workspace_result_still_renders_one_empty_gql_settlement() {
    let settlement = SearchTopologySettlement::from_workspace_result(
        "search-request-empty",
        &serde_json::json!({
            "schemaId": "agent.semantic-protocols.workspace-search-playbook-result",
            "schemaVersion": "1",
            "result": "no-match",
            "evidenceItemLimit": 30,
            "evidence": []
        }),
        &valid_library(),
    )
    .expect("empty Search is a ready settlement");
    assert_eq!(settlement.as_json()["resultState"], "empty");
    assert!(settlement.as_json().get("materializationSet").is_none());
    let rendered = settlement.render_org_gql().unwrap();
    assert_eq!(rendered.matches("#+begin_src gql").count(), 1);
    assert_eq!(rendered.matches("#+end_src").count(), 1);
}

#[test]
fn refinement_required_renders_one_incomplete_gql_settlement() {
    let settlement = SearchTopologySettlement::from_workspace_result(
        "search-request-refinement",
        &serde_json::json!({
            "schemaId": "agent.semantic-protocols.workspace-search-playbook-result",
            "schemaVersion": "1",
            "result": "refinement-required",
            "evidenceItemLimit": 30,
            "evidence": []
        }),
        &valid_library(),
    )
    .expect("refinement-required must remain an inspectable incomplete settlement");

    assert_eq!(settlement.as_json()["resultState"], "incomplete");
    assert_eq!(settlement.as_json()["terminal"]["state"], "incomplete");
    assert_eq!(
        settlement.as_json()["terminal"]["reasonKind"],
        "search-acquisition-incomplete"
    );
    assert!(settlement.as_json().get("materializationSet").is_none());
    let rendered = settlement.render_org_gql().expect("incomplete Org/GQL");
    assert!(rendered.starts_with("#+begin_src gql :name result\n"));
    assert_eq!(rendered.matches("#+begin_src gql").count(), 1);
}

#[test]
fn provider_contract_failure_cannot_publish_a_ready_settlement() {
    let error = SearchTopologySettlement::from_workspace_result(
        "search-request-provider-failure",
        &serde_json::json!({
            "schemaId": "agent.semantic-protocols.workspace-search-playbook-result",
            "schemaVersion": "1",
            "result": "provider-contract-failure",
            "evidenceItemLimit": 30,
            "evidence": []
        }),
        &valid_library(),
    )
    .expect_err("provider failure must remain a typed terminal");

    assert_eq!(error.reason_kind(), "search-provider-contract-failure");
}

#[test]
fn workspace_result_cannot_mint_a_selector_absent_from_attached_topology() {
    let error = SearchTopologySettlement::from_workspace_result(
        "search-request-stale",
        &serde_json::json!({
            "schemaId": "agent.semantic-protocols.workspace-search-playbook-result",
            "schemaVersion": "1",
            "result": "exact-selector-ready",
            "evidenceItemLimit": 30,
            "evidence": [{
                "owner": "src/missing.rs",
                "item": "function/missing",
                "selector": "rust://src/missing.rs#item/function/missing",
                "matchedBy": ["native-syntax:0"],
                "relation": "native-selector",
                "hit": {"native": true}
            }]
        }),
        &valid_library(),
    )
    .expect_err("flat evidence cannot bypass topology authority");
    assert_eq!(error.reason_kind(), "search-selector-absent-from-topology");
}
