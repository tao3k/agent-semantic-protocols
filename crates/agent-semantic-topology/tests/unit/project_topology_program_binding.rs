use std::collections::BTreeMap;

use agent_semantic_topology::admit_project_topology_program_bundle;
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

#[test]
fn topology_program_decodes_the_real_mrr_reasoning_bundle_identity() {
    let program = valid_program();
    let identity = admit_project_topology_program_bundle(&program, &admitted_receipts(&program))
        .expect("MRR bundle binding");
    assert!(identity.to_string().starts_with("mrr:reasoning-bundle:v1:"));
}

#[test]
fn topology_program_rejects_a_content_digest_in_the_mrr_identity_domain() {
    let mut program = valid_program();
    let receipts = admitted_receipts(&program);
    program["binding"]["mrrBundleIdentity"] = program["binding"]["schemeProgramDigest"].clone();
    let error = admit_project_topology_program_bundle(&program, &receipts)
        .expect_err("a content digest cannot impersonate an MRR bundle identity");
    assert_eq!(error.reason_kind(), "topology-mrr-bundle-identity-invalid");
}

#[test]
fn topology_program_requires_the_exact_independently_admitted_compilation_receipt() {
    let program = valid_program();
    let error = admit_project_topology_program_bundle(&program, &BTreeMap::new())
        .expect_err("an embedded receipt cannot authorize itself");
    assert_eq!(
        error.reason_kind(),
        "topology-compilation-receipt-unadmitted"
    );
}
