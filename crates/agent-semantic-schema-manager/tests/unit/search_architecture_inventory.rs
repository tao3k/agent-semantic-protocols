// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use crate::SearchArchitectureEdgeKind;
use crate::SearchArchitectureFactEdge;
use crate::SearchArchitectureFactInventory;
use crate::SearchArchitectureFactNode;
use crate::SearchArchitectureInventoryError;

fn node(id: &str, owner: &str, capabilities: &[&str]) -> SearchArchitectureFactNode {
    SearchArchitectureFactNode {
        node_id: id.to_owned(),
        source_selector: format!("rust://fixture#item/{id}"),
        declared_owner: owner.to_owned(),
        capability_claims: capabilities
            .iter()
            .map(|value| (*value).to_owned())
            .collect(),
    }
}

#[test]
fn neutral_fact_inventory_is_canonical_and_content_addressed() {
    let first = SearchArchitectureFactInventory::from_facts(
        vec!["client/default".to_owned(), "client/default".to_owned()],
        vec!["generated-client".to_owned()],
        vec![
            node(
                "retired-fallback",
                "retired-client-fallback",
                &["retry-policy"],
            ),
            node("generated-client", "generated-client", &["frame-codec"]),
        ],
        vec![SearchArchitectureFactEdge {
            source_node_id: "generated-client".to_owned(),
            target_node_id: "retired-fallback".to_owned(),
            kind: SearchArchitectureEdgeKind::RustReexport,
            enabled: true,
            evidence_selector: "rust://fixture#item/reexport/retired-fallback".to_owned(),
        }],
    )
    .expect("canonical inventory");
    let second = SearchArchitectureFactInventory::from_facts(
        vec!["client/default".to_owned()],
        vec!["generated-client".to_owned()],
        first.nodes.iter().cloned().rev().collect(),
        first.edges.clone(),
    )
    .expect("same canonical inventory");

    assert_eq!(first, second);
    assert!(first.inventory_digest.starts_with("blake3-256:"));
    assert_eq!(first.nodes[0].node_id, "generated-client");
    assert_eq!(first.nodes[1].declared_owner, "retired-client-fallback");
}

#[test]
fn neutral_fact_inventory_does_not_classify_owner_lifecycle() {
    let inventory = SearchArchitectureFactInventory::from_facts(
        vec![],
        vec!["old-path".to_owned()],
        vec![node(
            "old-path",
            "retired-executable-fixture",
            &["generation-authority"],
        )],
        vec![],
    )
    .expect("fact producer records declarations without judging them");

    assert_eq!(
        inventory.nodes[0].declared_owner,
        "retired-executable-fixture"
    );
    assert_eq!(
        inventory.nodes[0].capability_claims,
        ["generation-authority"]
    );
}

#[test]
fn inventory_fails_closed_on_an_unresolved_edge() {
    let error = SearchArchitectureFactInventory::from_facts(
        vec![],
        vec!["client".to_owned()],
        vec![node("client", "generated-client", &[])],
        vec![SearchArchitectureFactEdge {
            source_node_id: "client".to_owned(),
            target_node_id: "missing".to_owned(),
            kind: SearchArchitectureEdgeKind::RuntimeRoute,
            enabled: true,
            evidence_selector: "rust://fixture#item/route/missing".to_owned(),
        }],
    )
    .expect_err("unresolved edge must not enter a proof inventory");

    assert_eq!(
        error,
        SearchArchitectureInventoryError::UnknownNodeReference {
            field: "targetNodeId",
            node_id: "missing".to_owned(),
        }
    );
}

#[test]
fn inventory_rejects_a_fact_that_cannot_validate_against_the_shared_schema() {
    let error = SearchArchitectureFactInventory::from_facts(
        vec![],
        vec!["client with spaces".to_owned()],
        vec![node("client with spaces", "generated-client", &[])],
        vec![],
    )
    .expect_err("invalid shared identifier must be rejected before publication");

    assert_eq!(
        error,
        SearchArchitectureInventoryError::InvalidIdentifier {
            field: "architecture identifier",
            value: "client with spaces".to_owned(),
        }
    );
}
