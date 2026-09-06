// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-only

use std::path::Path;
use std::sync::Arc;

use agent_semantic_provider_protocol::ProviderRegistrationDocument;
use serde_json::json;

use super::ActiveProviderMember;
use super::ActiveProviderProjectionDocument;
use super::RuntimeActiveProviderProjection;
use super::SCHEMA_ID;
use super::SCHEMA_VERSION;
use super::generation;
use super::load_runtime_active_provider_projection;
use super::provider_languages_for_generation_demand;
use super::runtime_source_index_provider_projection;
use super::workspace_required_provider_languages_for_paths;

#[test]
fn runtime_source_index_projection_is_derived_from_live_register() {
    let registration = ProviderRegistrationDocument {
        language_id: "rust".to_owned(),
        provider_id: "asp-rust".to_owned(),
        registration: json!({
            "languageId": "rust",
            "providerId": "asp-rust",
            "namespace": "agent.semantic-protocols.languages.rust",
            "sourceInventory": {
                "packageRoots": ["crates"],
                "configFiles": ["Cargo.toml"],
                "sourceExtensions": [".rs"],
                "projectResolution": {"entryMarkers": ["Cargo.toml"]},
                "documentResolution": null
            },
            "searchCapabilities": {
                "ownerItems": true,
                "semanticFacts": true,
                "dependencyTopology": true,
                "dependencyTopologyMetadata": true
            },
            "queryPackDescriptor": {
                "descriptorId": "rust.search",
                "descriptorVersion": "1",
                "languageId": "rust",
                "termRoleOverrides": [],
                "recipes": []
            },
            "runtimeContract": {
                "transport": "http-json",
                "aspClientServer": {
                    "schemaId": "agent.semantic-protocols.asp-client-server-descriptor",
                    "schemaVersion": "1",
                    "transport": "http-json",
                    "command": ["serve"],
                    "healthPath": "/health",
                    "requestPath": "/v1/provider-runtime",
                    "shutdownPath": "/shutdown",
                    "warmupPolicy": "before-ready"
                },
                "operations": [{
                    "operation": "projection-batch",
                    "requestSchema": {
                        "schemaId": "agent.semantic-protocols.provider-language-projection-batch-request",
                        "schemaVersion": "1"
                    },
                    "responseSchema": {
                        "schemaId": "agent.semantic-protocols.provider-language-projection-batch-response",
                        "schemaVersion": "1"
                    }
                }]
            },
            "routes": [{
                "schemaId": "agent.semantic-protocols.provider-route",
                "schemaVersion": "1",
                "routeId": "rust.search",
                "operation": "search",
                "authority": "asp-server",
                "target": {"languageId": "rust", "providerId": "asp-rust"},
                "inputs": [],
                "requirements": [],
                "effects": {
                    "access": "read",
                    "idempotent": true,
                    "cancellable": true,
                    "concurrency": "shared-read",
                    "streaming": false
                },
                "output": {
                    "schema": {
                        "schemaId": "agent.semantic-protocols.search-packet",
                        "schemaVersion": "1"
                    },
                    "mediaType": "application/json"
                },
                "failureSchemaIds": ["agent.semantic-protocols.route-failure"],
                "cache": {
                    "authority": "asp-server",
                    "scope": "workspace",
                    "keySlots": []
                },
                "telemetry": {
                    "spanName": "asp.route.search",
                    "attributeSlots": []
                }
            }]
        }),
    };
    let mut inactive_julia_registration = registration.clone();
    inactive_julia_registration.language_id = "julia".to_owned();
    inactive_julia_registration.provider_id = "asp-julia".to_owned();
    inactive_julia_registration.registration["languageId"] = json!("julia");
    inactive_julia_registration.registration["providerId"] = json!("asp-julia");
    inactive_julia_registration.registration["namespace"] =
        json!("agent.semantic-protocols.languages.julia");
    inactive_julia_registration.registration["sourceInventory"]["sourceExtensions"] =
        json!([".jl"]);
    inactive_julia_registration.registration["sourceInventory"]["projectResolution"]["entryMarkers"] =
        json!(["Project.toml"]);
    inactive_julia_registration.registration["queryPackDescriptor"]["languageId"] = json!("julia");
    inactive_julia_registration.registration["routes"][0]["routeId"] = json!("julia.search");
    inactive_julia_registration.registration["routes"][0]["target"]["languageId"] = json!("julia");
    inactive_julia_registration.registration["routes"][0]["target"]["providerId"] =
        json!("asp-julia");
    let closure_register =
        agent_semantic_client_db::runtime_provider_register::RuntimeProviderRegister::from_seed(
            vec![registration.clone(), inactive_julia_registration.clone()],
        )
        .expect("closure register");
    let required = workspace_required_provider_languages_for_paths(
        &closure_register,
        [Path::new("Cargo.toml"), Path::new("src/lib.rs")],
    )
    .expect("workspace provider closure");
    assert_eq!(
        required,
        std::collections::BTreeSet::from(["rust".to_owned()])
    );
    let targeted = provider_languages_for_generation_demand(
        &closure_register,
        &["Cargo.toml".to_owned(), "Project.toml".to_owned()],
        Some(
            &agent_semantic_client_db::runtime_server_admission::WorkspaceGenerationProviderTarget {
                language_id: "rust".to_owned(),
                provider_id: Some("asp-rust".to_owned()),
            },
        ),
    )
    .expect("targeted provider closure");
    assert_eq!(
        targeted,
        std::collections::BTreeSet::from(["rust".to_owned()])
    );
    let incomplete_register =
        agent_semantic_client_db::runtime_provider_register::RuntimeProviderRegister::from_seed(
            vec![registration.clone(), inactive_julia_registration],
        )
        .expect("live register");
    let providers = vec![ActiveProviderMember {
        language_id: "rust".to_owned(),
        provider_id: "asp-rust".to_owned(),
        materialized_path: "/runtime/artifacts/asp-rust".to_owned(),
        artifact_digest: format!("blake3-256:{}", "a".repeat(64)),
        artifact_metadata_digest: format!("blake3-256:{}", "b".repeat(64)),
        execution_command_digest: format!("sha256:{}", "c".repeat(64)),
    }];
    let artifacts = RuntimeActiveProviderProjection {
        document: Arc::new(ActiveProviderProjectionDocument {
            schema_id: SCHEMA_ID.to_owned(),
            schema_version: SCHEMA_VERSION.to_owned(),
            generation: generation(&providers).expect("generation"),
            providers,
        }),
        active_bundle_digest: format!("blake3-256:{}", "d".repeat(64)),
    };

    let rust_only = std::collections::BTreeSet::from(["rust".to_owned()]);
    let (rust_projection, _) =
        runtime_source_index_provider_projection(&artifacts, &incomplete_register, &rust_only)
            .expect("an unused Julia provider must not block a Rust workspace");
    assert_eq!(rust_projection.providers.len(), 1);
    assert_eq!(rust_projection.providers[0].language_id.as_str(), "rust");

    let rust_and_julia = std::collections::BTreeSet::from(["julia".to_owned(), "rust".to_owned()]);
    let incomplete_error =
        runtime_source_index_provider_projection(&artifacts, &incomplete_register, &rust_and_julia)
            .expect_err("a required Julia provider must reject a missing artifact");
    assert!(
        incomplete_error.contains("missing=julia:artifact"),
        "{incomplete_error}"
    );

    let register =
        agent_semantic_client_db::runtime_provider_register::RuntimeProviderRegister::from_seed(
            vec![registration],
        )
        .expect("complete live register");
    let (projection, generation) =
        runtime_source_index_provider_projection(&artifacts, &register, &rust_only)
            .expect("complete Runtime source-index provider projection");
    assert!(
        projection
            .authority_ref
            .starts_with("runtime-provider-register:sha256:")
    );
    assert_eq!(projection.providers.len(), 1);
    let provider = &projection.providers[0];
    assert_eq!(provider.provider_id.as_str(), "asp-rust");
    assert_eq!(provider.binary, "/runtime/artifacts/asp-rust");
    assert_eq!(provider.source_extensions, [".rs"]);
    assert!(provider.registration_digest.starts_with("sha256:"));
    assert!(generation.starts_with("sha256:"));

    let mut relocated_document = (*artifacts.document).clone();
    relocated_document.providers[0].materialized_path =
        "/different-state-home/runtime/artifacts/asp-rust".to_owned();
    relocated_document.generation =
        super::generation(&relocated_document.providers).expect("relocated legacy projection");
    let relocated = RuntimeActiveProviderProjection {
        document: Arc::new(relocated_document),
        active_bundle_digest: artifacts.active_bundle_digest.clone(),
    };
    let (_, relocated_closure) =
        runtime_source_index_provider_projection(&relocated, &register, &rust_only)
            .expect("relocated provider closure");
    assert_eq!(
        relocated_closure, generation,
        "workspace provider closure identity must not depend on an absolute compatibility path"
    );

    let launch = artifacts
        .runtime_launch(Path::new("/workspace"), "rust", &register)
        .expect("Runtime provider launch");
    assert_eq!(
        launch.spec.env["ASP_PROVIDER_RUNTIME_CONTRACT_DIGEST"],
        launch.expected_receipt.contract_digest
    );
    let launched_operations: Vec<
        agent_semantic_provider_transport::ProviderRuntimeContractOperation,
    > = serde_json::from_str(&launch.spec.env["ASP_PROVIDER_RUNTIME_OPERATIONS_JSON"])
        .expect("decode launched provider operations");
    assert_eq!(launched_operations, launch.expected_receipt.operations);
}

#[cfg(unix)]
#[tokio::test]
async fn runtime_provider_projection_is_derived_only_from_the_verified_active_bundle() {
    let root = std::env::temp_dir().join(format!(
        "asp-active-provider-projection-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&root);
    let sources = root.join("sources");
    std::fs::create_dir_all(&sources).expect("create bundle sources");
    let asp = sources.join("asp");
    let hook = sources.join("asp-hook");
    let provider = sources.join("asp-rust");
    std::fs::write(&asp, b"asp-v1").expect("write asp fixture");
    std::fs::write(&hook, b"hook-v1").expect("write hook fixture");
    std::fs::write(&provider, b"provider-v1").expect("write provider fixture");
    let members = [
        agent_semantic_artifacts::runtime_artifact_publication::RuntimeArtifactBundleMemberSource {
            name: "asp-hook",
            source: &hook,
        },
        agent_semantic_artifacts::runtime_artifact_publication::RuntimeArtifactBundleMemberSource {
            name: "asp-rust",
            source: &provider,
        },
    ];
    let publication = agent_semantic_artifacts::runtime_artifact_publication::publish_runtime_artifact_bundle_members(
        &root,
        &asp,
        &root.join("runtime/bin/asp"),
        "dev",
        &members,
    )
    .await
    .expect("publish active Runtime bundle");

    let artifacts = load_runtime_active_provider_projection(&root)
        .await
        .expect("derive provider projection from active bundle");
    assert_eq!(artifacts.generation(), publication.bundle_digest.as_str());
    assert!(artifacts.document.providers.iter().any(|provider| {
        provider.language_id == "rust"
            && provider.provider_id == "asp-rust"
            && Path::new(&provider.materialized_path).starts_with(
                agent_semantic_artifacts::RuntimeArtifactStateLayout::new(&root)
                    .generation_store()
                    .canonicalize()
                    .expect("canonical generation store"),
            )
    }));
    assert!(
        !root
            .join("runtime/installed-provider-artifacts.json")
            .exists()
    );
    assert!(
        !root
            .join("runtime/installed-provider-binding.v1.json")
            .exists()
    );

    std::fs::remove_dir_all(root).expect("remove active provider projection fixture");
}
