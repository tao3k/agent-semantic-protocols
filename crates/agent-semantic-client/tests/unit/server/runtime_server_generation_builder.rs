// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use super::runtime_schema_bundle_digest;

fn digest(label: &str) -> agent_semantic_artifacts::blake3_content_digest::Blake3ContentDigest {
    agent_semantic_artifacts::blake3_content_digest::Blake3ContentDigest::from_bytes(
        label.as_bytes(),
    )
}

fn execution_binding()
-> agent_semantic_artifacts::runtime_artifact_slots::RuntimeArtifactBundleBinding {
    agent_semantic_artifacts::runtime_artifact_slots::RuntimeArtifactBundleBinding::new(
        digest("provider-registration"),
        digest("provider-artifacts"),
        digest("evaluator-policy"),
        digest("evaluator-abi"),
        digest("global-schema-bundle"),
    )
}

#[test]
fn generation_uses_global_runtime_schema_identity_after_validating_language_subset() {
    let schemas = agent_semantic_runtime_server::RuntimeSchemaBundleCatalog::load_embedded()
        .expect("embedded schemas");
    let binding = execution_binding();
    let observed = runtime_schema_bundle_digest(&schemas, ["rust"], &binding)
        .expect("required Rust schemas are embedded");

    assert_eq!(observed, binding.schema_bundle_digest().as_str());
}

#[test]
fn generation_rejects_a_language_without_an_embedded_schema_bundle() {
    let schemas = agent_semantic_runtime_server::RuntimeSchemaBundleCatalog::load_embedded()
        .expect("embedded schemas");
    let error = runtime_schema_bundle_digest(&schemas, ["not-a-language"], &execution_binding())
        .expect_err("unknown schema language must fail closed");

    assert!(error.contains("absent for required language"));
}
