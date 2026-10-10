// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use std::collections::BTreeMap;

use agent_semantic_topology::{ProjectTopologyLibrary, ProjectTopologyManifest};
use serde_json::{Value, json};

fn packet() -> Value {
    serde_json::from_str(include_str!(
        "../../../../schemas/fixtures/project-topology-library/valid-polyglot.v1.json"
    ))
    .expect("valid Project Topology fixture")
}

fn admit(packet: Value) -> Result<ProjectTopologyLibrary, String> {
    let manifest = ProjectTopologyManifest::parse_org(include_str!(
        "../../../../org/templates/project.workspace-manifest.v1.org"
    ))
    .expect("Project Topology manifest");
    let receipt = packet["fromScratchRebuildReceipt"].clone();
    let receipts = BTreeMap::from([("topology-rebuild-1".to_owned(), receipt)]);
    ProjectTopologyLibrary::admit_with_receipts(packet, manifest.project_workspace(), &receipts)
        .map_err(|error| error.reason_kind().to_owned())
}

fn unresolved(packet: &mut Value, coverage: &str) {
    packet["expectedRelations"] = json!([{
        "id": "expected-config",
        "anchor": "registry",
        "target": "refresh",
        "relation": "READS_CONFIG",
        "targetKind": "Method",
        "depth": 1,
        "coverage": coverage
    }]);
    let (state, reason) = match coverage {
        "none" => ("unknown", "binding-not-established"),
        "partial" => ("unknown", "coverage-open"),
        "complete" => ("certified-missing", "complete-coverage-no-witness"),
        _ => unreachable!("test coverage is known"),
    };
    packet["frontiers"] = json!([{
        "anchor": "registry",
        "target": "refresh",
        "relation": "READS_CONFIG",
        "targetKind": "Method",
        "depth": 1,
        "state": state,
        "reason": reason
    }]);
    if coverage != "none" {
        packet["frontiers"][0]["coverageRef"] = json!("coverage-config");
        packet["coverageCertificates"] = json!([{
            "id": "coverage-config",
            "relation": "READS_CONFIG",
            "targetKind": "Method",
            "scope": coverage,
            "digest": "blake3-256:9999999999999999999999999999999999999999999999999999999999999999"
        }]);
    }
}

#[test]
fn unresolved_relation_truth_table_is_admitted() {
    for coverage in ["none", "partial", "complete"] {
        let mut packet = packet();
        unresolved(&mut packet, coverage);
        admit(packet).expect("truth-table frontier should be admitted");
    }
}

#[test]
fn frontier_reason_must_match_coverage_truth_table() {
    let mut packet = packet();
    unresolved(&mut packet, "partial");
    packet["frontiers"][0]["reason"] = json!("binding-not-established");
    assert_eq!(
        admit(packet).expect_err("wrong reason must fail"),
        "topology-frontier-classification-mismatch"
    );
}

#[test]
fn proposed_edge_does_not_close_source_owned_expectation() {
    let mut packet = packet();
    unresolved(&mut packet, "none");
    let semantic_digest = packet["identities"]["semanticTopologyDigest"].clone();
    packet["edges"].as_array_mut().expect("edges").push(json!({
        "id": "proposed-config",
        "segmentId": null,
        "from": "registry",
        "to": "refresh",
        "relation": "READS_CONFIG",
        "modality": "proposed",
        "bindingDigest": semantic_digest,
        "witnesses": ["model-proposal-1"]
    }));
    admit(packet).expect("proposal leaves the source-owned frontier open");
}

#[test]
fn coverage_certificate_cannot_float_free_of_frontier() {
    let mut packet = packet();
    packet["coverageCertificates"] = json!([{
        "id": "coverage-unused",
        "relation": "READS_CONFIG",
        "targetKind": "Method",
        "scope": "partial",
        "digest": "blake3-256:8888888888888888888888888888888888888888888888888888888888888888"
    }]);
    assert_eq!(
        admit(packet).expect_err("unused coverage must fail"),
        "topology-frontier-coverage-extraneous"
    );
}
