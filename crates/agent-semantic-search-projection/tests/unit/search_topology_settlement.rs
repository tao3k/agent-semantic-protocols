// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use agent_semantic_search_projection::SearchTopologySettlement;
use agent_semantic_topology::{ProjectTopologyLibrary, ProjectTopologyManifest};
use std::collections::BTreeMap;

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
        "../../../../org/templates/project.topology-program.v1.org"
    ))
    .expect("Project Topology manifest");
    ProjectTopologyLibrary::admit_with_receipts(packet, manifest.project_workspace(), &receipts)
        .expect("admitted Project Topology fixture")
}

#[test]
fn topology_settlement_admits_the_bound_single_gql_fixture() {
    let settlement = SearchTopologySettlement::admit(valid_settlement())
        .expect("the shared settlement fixture must be admitted");

    assert_eq!(settlement.materialization_request_id(), "query-playbook-1");
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
fn topology_settlement_rejects_unstable_materialization_order() {
    let mut packet = valid_settlement();
    packet["materializationSet"]["selectors"]
        .as_array_mut()
        .expect("selectors")
        .reverse();

    let error = SearchTopologySettlement::admit(packet)
        .expect_err("query playbook materialization must be deterministic");
    assert_eq!(error.reason_kind(), "materialization-order");
}

#[test]
fn topology_settlement_rejects_unavailable_materialization_selectors() {
    let mut packet = valid_settlement();
    packet["materializationSet"]["selectors"][0] =
        serde_json::json!("org://docs/missing.org#item/heading/Missing");

    let error = SearchTopologySettlement::admit(packet)
        .expect_err("query playbook cannot materialize a selector absent from its nodes");
    assert_eq!(error.reason_kind(), "materialization-selector-unavailable");
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
fn topology_settlement_renders_one_compact_polyglot_gql_result() {
    let settlement = SearchTopologySettlement::admit(valid_settlement()).unwrap();
    let rendered = settlement
        .render_org_gql()
        .expect("render admitted settlement");

    assert_eq!(rendered.matches("#+begin_src gql").count(), 1);
    assert_eq!(rendered.matches("#+end_src").count(), 1);
    assert!(!rendered.contains("#+begin_src ascent"));
    assert!(rendered.contains("(lang_rust:Language {id:\"rust\"})-[:RESULTS]->["));
    assert!(rendered.contains("refresh:RustMethod"));
    assert!(rendered.contains("selector:\"rust://src/registry.rs#item/method/refresh_registry/scope/implementation-owner/type/Registry\""));
    assert!(rendered.contains("projection:{rank:1,depth:0,hit:{fd:true,rg:[[42,46]],tantivy:[\"artifact refresh\"],native:true}}"));
    assert!(rendered.contains("(lang_org:Language {id:\"org\"})-[:RESULTS]->["));
    assert!(rendered.contains("meaning:SemanticAnnotation"));
    assert!(rendered.contains("state:\"proposed\""));
    assert!(rendered.contains("(refresh)-[:DOCUMENTED_PATH {modality:\"derived\""));
    assert!(rendered.contains("proof:\"proof-path-42\""));
    assert!(rendered.contains("materialize:MaterializationSet"));
    assert!(rendered.contains("request_id:\"query-playbook-1\""));
}

#[test]
fn executed_workspace_result_is_joined_to_topology_before_rendering() {
    let library = valid_library();
    let workspace_result = serde_json::json!({
        "schemaId": "agent.semantic-protocols.workspace-search-playbook-result",
        "schemaVersion": "1",
        "result": "relationship-supported",
        "evidence": [{
            "owner": "src/registry.rs",
            "item": "method/refresh_registry/scope/implementation-owner/type/Registry",
            "selector": "rust://src/registry.rs#item/method/refresh_registry/scope/implementation-owner/type/Registry",
            "matchedBy": ["rg:0", "syntax:0", "graph:0"],
            "relation": "syntax-capture:method"
        }]
    });
    let settlement = SearchTopologySettlement::from_workspace_result(
        "search-request-1",
        &workspace_result,
        &library,
    )
    .expect("Runtime Search evidence joins the attached topology");

    assert_eq!(settlement.as_json()["resultState"], "materializable");
    assert_eq!(
        settlement.as_json()["materializationSet"]["selectors"][0],
        workspace_result["evidence"][0]["selector"]
    );
    assert!(
        settlement
            .render_org_gql()
            .unwrap()
            .contains("(registry)-[:DECLARES")
    );
}

#[test]
fn empty_workspace_result_still_renders_one_empty_gql_settlement() {
    let settlement = SearchTopologySettlement::from_workspace_result(
        "search-request-empty",
        &serde_json::json!({
            "schemaId": "agent.semantic-protocols.workspace-search-playbook-result",
            "schemaVersion": "1",
            "result": "no-match",
            "evidence": []
        }),
        &valid_library(),
    )
    .expect("empty Search is a ready settlement");
    assert_eq!(settlement.as_json()["resultState"], "empty");
    assert_eq!(settlement.as_json()["materializationSet"]["state"], "empty");
    let rendered = settlement.render_org_gql().unwrap();
    assert_eq!(rendered.matches("#+begin_src gql").count(), 1);
    assert_eq!(rendered.matches("#+end_src").count(), 1);
}

#[test]
fn workspace_result_cannot_mint_a_selector_absent_from_attached_topology() {
    let error = SearchTopologySettlement::from_workspace_result(
        "search-request-stale",
        &serde_json::json!({
            "schemaId": "agent.semantic-protocols.workspace-search-playbook-result",
            "schemaVersion": "1",
            "result": "exact-selector-ready",
            "evidence": [{
                "owner": "src/missing.rs",
                "item": "function/missing",
                "selector": "rust://src/missing.rs#item/function/missing",
                "matchedBy": ["native-syntax:0"],
                "relation": "native-selector"
            }]
        }),
        &valid_library(),
    )
    .expect_err("flat evidence cannot bypass topology authority");
    assert_eq!(error.reason_kind(), "search-selector-absent-from-topology");
}
