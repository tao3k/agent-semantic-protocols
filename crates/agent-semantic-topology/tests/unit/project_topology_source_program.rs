// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use agent_semantic_topology::{
    PROJECT_TOPOLOGY_STANDARD_PROGRAM_RESOURCE, ProjectTopologySourceProgram,
};

#[test]
fn orgize_projects_the_native_topology_program() {
    assert_eq!(
        PROJECT_TOPOLOGY_STANDARD_PROGRAM_RESOURCE,
        "org/templates/asp/topology/program.v1.org"
    );
    let program = ProjectTopologySourceProgram::standard().expect("native Org topology program");
    assert_eq!(program.program_id(), "mrr.topology.standard.v1");
    assert_eq!(program.engine_profile(), "ascent-seminaive.v1");
    assert_eq!(program.rules().len(), 1);
    assert!(program.expectations().is_empty());
    assert!(program.source_digest().starts_with("blake3-256:"));
    assert!(program.inference_program().is_ok());
}

#[test]
fn org_program_rejects_relation_erasure() {
    let source = include_str!("../../../../org/templates/asp/topology/program.v1.org")
        .replace(":RIGHT_RELATION: READS_CONFIG", ":RIGHT_RELATION: COVERS");
    let error = ProjectTopologySourceProgram::parse_org(&source)
        .expect_err("COVERS cannot impersonate READS_CONFIG");
    assert_eq!(
        error.reason_kind(),
        "topology-source-program-rule-unsupported"
    );
}

#[test]
fn native_program_is_not_loaded_from_project_agents_state() {
    assert!(!PROJECT_TOPOLOGY_STANDARD_PROGRAM_RESOURCE.starts_with(".agents/"));
}

#[test]
fn indexed_query_ignores_declarations_outside_the_program_subtree() {
    let source = format!(
        "{}\n* Unscoped Lookalikes\n:PROPERTIES:\n:RULE_ID: fake.rule\n:HEAD_RELATION: WRONG\n:LEFT_RELATION: WRONG\n:RIGHT_RELATION: WRONG\n:JOIN: wrong\n:EXPECTATION_ID: fake.expectation\n:END:\n",
        include_str!("../../../../org/templates/asp/topology/program.v1.org")
    );

    let program = ProjectTopologySourceProgram::parse_org(&source)
        .expect("out-of-scope properties are not topology declarations");
    assert_eq!(program.rules().len(), 1);
    assert!(program.expectations().is_empty());
}

#[test]
fn indexed_query_ignores_nested_program_identity_lookalikes() {
    let source = include_str!("../../../../org/templates/asp/topology/program.v1.org").replace(
        "** Depends On Config",
        "** Nested Lookalike\n:PROPERTIES:\n:TOPOLOGY_PROGRAM_ID: not-a-program\n:END:\n\n** Depends On Config",
    );

    let program = ProjectTopologySourceProgram::parse_org(&source)
        .expect("only a top-level indexed declaration owns program identity");
    assert_eq!(program.program_id(), "mrr.topology.standard.v1");
}

#[test]
fn indexed_expectation_changes_source_identity_and_typed_facts() {
    let source = include_str!("../../../../org/templates/asp/topology/program.v1.org");
    let changed = format!(
        "{source}\n** Expected Documentation\n:PROPERTIES:\n:EXPECTATION_ID: expected-documentation\n:ANCHOR_SELECTOR: rust://src/registry.rs#item/struct/Registry\n:TARGET_SELECTOR: org://docs/publication.org#item/heading/Publication\n:EXPECTED_RELATION: DOCUMENTS\n:TARGET_KIND: heading\n:DEPTH: 2\n:COVERAGE: partial\n:END:\n"
    );

    let before = ProjectTopologySourceProgram::parse_org(source).expect("standard program");
    let after = ProjectTopologySourceProgram::parse_org(&changed).expect("changed program");
    assert_eq!(after.expectations().len(), 1);
    assert_eq!(after.expectations()[0].relation(), "DOCUMENTS");
    assert_ne!(before.source_digest(), after.source_digest());
}

#[test]
fn v1_rejects_a_program_identity_that_cannot_name_the_executed_bundle() {
    let source = include_str!("../../../../org/templates/asp/topology/program.v1.org").replace(
        ":TOPOLOGY_PROGRAM_ID: mrr.topology.standard.v1",
        ":TOPOLOGY_PROGRAM_ID: mrr.topology.other.v1",
    );
    let error = ProjectTopologySourceProgram::parse_org(&source)
        .expect_err("V1 cannot relabel its fixed executable program");
    assert_eq!(
        error.reason_kind(),
        "topology-source-program-identity-unsupported"
    );
}

#[test]
fn contract_authority_must_be_document_scoped() {
    let source = include_str!("../../../../org/templates/asp/topology/program.v1.org")
        .replace(":CONTRACT_ORG:", ":IGNORED_CONTRACT_ORG:")
        + "\n* Lookalike Contract\n:PROPERTIES:\n:CONTRACT_ORG: [[program.v1.org][asp.topology.program.v1]]\n:END:\n";

    let error = ProjectTopologySourceProgram::parse_org(&source)
        .expect_err("a headline property cannot claim document contract authority");
    assert_eq!(
        error.reason_kind(),
        "topology-source-program-contract-mismatch"
    );
}
