// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use std::collections::BTreeMap;

use agent_semantic_topology::{ProjectTopologyManifest, admit_project_topology_program_bundle};
use serde_json::Value;

fn valid_program() -> Value {
    serde_json::from_str(include_str!(
        "../../../../schemas/fixtures/project-topology-program-binding/valid-project.v1.json"
    ))
    .expect("valid fixture")
}

fn admitted_receipts(program: &Value) -> BTreeMap<String, Value> {
    let receipt = program["program"]["compilationReceipt"].clone();
    BTreeMap::from([(
        receipt["id"].as_str().expect("receipt id").to_owned(),
        receipt,
    )])
}

fn manifest() -> ProjectTopologyManifest {
    ProjectTopologyManifest::parse_org(include_str!(
        "../../../../org/templates/project.topology-program.v1.org"
    ))
    .expect("Project Topology manifest")
}

#[test]
fn topology_program_decodes_the_real_mrr_reasoning_bundle_identity() {
    let program = valid_program();
    let manifest = manifest();
    let identity = admit_project_topology_program_bundle(
        &program,
        manifest.project_workspace(),
        &admitted_receipts(&program),
    )
    .expect("MRR bundle binding");
    assert!(identity.to_string().starts_with("mrr:reasoning-bundle:v1:"));
}

#[test]
fn topology_program_rejects_a_content_digest_in_the_mrr_identity_domain() {
    let mut program = valid_program();
    let receipts = admitted_receipts(&program);
    let manifest = manifest();
    program["binding"]["mrrBundleIdentity"] = program["binding"]["schemeProgramDigest"].clone();
    let error =
        admit_project_topology_program_bundle(&program, manifest.project_workspace(), &receipts)
            .expect_err("a content digest cannot impersonate an MRR bundle identity");
    assert_eq!(error.reason_kind(), "topology-mrr-bundle-identity-invalid");
}

#[test]
fn topology_program_requires_the_exact_independently_admitted_compilation_receipt() {
    let program = valid_program();
    let manifest = manifest();
    let error = admit_project_topology_program_bundle(
        &program,
        manifest.project_workspace(),
        &BTreeMap::new(),
    )
    .expect_err("an embedded receipt cannot authorize itself");
    assert_eq!(
        error.reason_kind(),
        "topology-compilation-receipt-unadmitted"
    );
}

#[test]
fn topology_program_rejects_runtime_generated_workspace_identity() {
    let mut program = valid_program();
    let receipts = admitted_receipts(&program);
    let manifest = manifest();
    program["binding"]["projectWorkspace"]["projectWorkspaceIdentity"] =
        Value::String("workspace-example".to_owned());

    let error =
        admit_project_topology_program_bundle(&program, manifest.project_workspace(), &receipts)
            .expect_err("a Runtime workspace id cannot replace GitOps project identity");
    assert_eq!(error.reason_kind(), "topology-project-workspace-invalid");
}

#[test]
fn topology_program_rejects_another_valid_workspace_not_declared_by_manifest() {
    let mut program = valid_program();
    let receipts = admitted_receipts(&program);
    let manifest = manifest();
    program["binding"]["projectWorkspace"]["projectWorkspaceIdentity"] =
        Value::String("git+https://github.com/tao3k/another-project.git#workspace/root".to_owned());

    let error =
        admit_project_topology_program_bundle(&program, manifest.project_workspace(), &receipts)
            .expect_err("syntax-valid identity cannot bypass the manifest authority");
    assert_eq!(error.reason_kind(), "topology-project-workspace-mismatch");
}
