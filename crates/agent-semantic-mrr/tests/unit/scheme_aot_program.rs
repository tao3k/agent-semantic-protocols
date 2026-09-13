// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use std::collections::BTreeMap;

use agent_semantic_mrr::admit_scheme_aot_program_binding;
use serde_json::{Value, json};

fn binding_and_receipt() -> (
    serde_json::Map<String, Value>,
    serde_json::Map<String, Value>,
) {
    let bundle = format!("mrr:reasoning-bundle:v1:{}", "a".repeat(64));
    let binding = json!({
        "schemeProgramDigest": format!("blake3-256:{}", "b".repeat(64)),
        "compiledProgramAbiDigest": format!("blake3-256:{}", "c".repeat(64)),
        "mrrBundleIdentity": bundle,
    })
    .as_object()
    .expect("binding")
    .clone();
    let receipt = json!({
        "schemaId": "agent.semantic-protocols.mrr-program-compilation-receipt",
        "schemaVersion": "1",
        "id": "receipt-1",
        "state": "admitted",
        "schemeProgramDigest": binding["schemeProgramDigest"],
        "compiledProgramAbiDigest": binding["compiledProgramAbiDigest"],
        "mrrBundleIdentity": binding["mrrBundleIdentity"],
    })
    .as_object()
    .expect("receipt")
    .clone();
    (binding, receipt)
}

#[test]
fn exact_independently_admitted_scheme_aot_tuple_is_decoded() {
    let (binding, receipt) = binding_and_receipt();
    let admitted = BTreeMap::from([("receipt-1".to_owned(), Value::Object(receipt.clone()))]);
    let result = admit_scheme_aot_program_binding(&binding, &receipt, &admitted)
        .expect("admitted Scheme AOT binding");
    assert!(
        result
            .mrr_bundle_identity()
            .to_string()
            .starts_with("mrr:reasoning-bundle:v1:")
    );
}

#[test]
fn embedded_receipt_cannot_admit_itself() {
    let (binding, receipt) = binding_and_receipt();
    let error = admit_scheme_aot_program_binding(&binding, &receipt, &BTreeMap::new())
        .expect_err("receipt must be independently admitted");
    assert_eq!(error.reason_kind(), "mrr-compilation-receipt-unadmitted");
}
