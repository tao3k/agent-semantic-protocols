// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use super::{normalized_changed_source_owners, runtime_schema_bundle_digest};

fn rust_projection() -> agent_semantic_client_core::RuntimeProviderProjection {
    agent_semantic_client_core::RuntimeProviderProjection {
        authority_ref: "runtime:test".to_owned(),
        providers: vec![agent_semantic_client_core::RuntimeProvider {
            registration_digest: "sha256:test".to_owned(),
            execution_artifact_digest: "blake3-256:test".to_owned(),
            namespace: "agent.semantic-protocols.languages.rust".to_owned(),
            language_id: "rust".into(),
            provider_id: "asp-rust".into(),
            binary: "asp:rust".to_owned(),
            package_roots: vec![".".to_owned()],
            config_files: vec!["Cargo.toml".to_owned()],
            source_extensions: vec![".rs".to_owned()],
            source_inventory_capabilities:
                agent_semantic_client_core::ProviderSourceInventoryCapabilities {
                    project_resolution: None,
                    document_resolution: None,
                },
            search_capabilities: serde_json::from_value(serde_json::json!({
                "ownerItems": true,
                "semanticFacts": true,
                "dependencyTopology": true,
                "dependencyTopologyMetadata": true
            }))
            .expect("search capabilities"),
            query_pack_descriptor: serde_json::from_value(serde_json::json!({
                "descriptorId": "rust.search",
                "descriptorVersion": "1",
                "languageId": "rust",
                "termRoleOverrides": [],
                "recipes": []
            }))
            .expect("query pack descriptor"),
            semantic_facts_descriptor: None,
            runtime_operations: Vec::new(),
        }],
    }
}

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

#[test]
fn hook_source_mutation_retains_the_exact_owner_cut() {
    let workspace = tempfile::tempdir().expect("temporary workspace");
    let root = workspace.path();
    std::fs::create_dir_all(root.join("src")).expect("source directory");
    std::fs::write(root.join("src/lib.rs"), "pub fn changed() {}\n").expect("changed owner");
    let changed = [std::path::PathBuf::from("src/lib.rs")]
        .into_iter()
        .collect();

    let owners = normalized_changed_source_owners(root, &changed, &rust_projection())
        .expect("normalized source-owner mutation");

    assert_eq!(owners, ["src/lib.rs".to_owned()].into_iter().collect());
}

#[test]
fn auxiliary_mutation_fails_closed_to_complete_generation() {
    let workspace = tempfile::tempdir().expect("temporary workspace");
    let root = workspace.path();
    let changed = [std::path::PathBuf::from("Cargo.toml")]
        .into_iter()
        .collect();

    let owners = normalized_changed_source_owners(root, &changed, &rust_projection())
        .expect("normalized auxiliary mutation");

    assert!(owners.is_empty());
}

#[test]
fn removed_owner_fails_closed_until_tombstone_delta_publication_is_wired() {
    let workspace = tempfile::tempdir().expect("temporary workspace");
    let changed = [std::path::PathBuf::from("src/removed.rs")]
        .into_iter()
        .collect();

    let owners = normalized_changed_source_owners(workspace.path(), &changed, &rust_projection())
        .expect("normalized removed owner");

    assert!(owners.is_empty());
}

#[test]
fn mutation_path_outside_workspace_is_rejected() {
    let root = std::path::Path::new("/workspace/project");
    let changed = [std::path::PathBuf::from("/other/project/src/lib.rs")]
        .into_iter()
        .collect();

    let error = normalized_changed_source_owners(root, &changed, &rust_projection())
        .expect_err("escaped mutation path");

    assert!(error.contains("escaped project root"));
}
