use std::{path::Path, sync::Arc};

use agent_semantic_provider_protocol::ProviderRegistrationDocument;
use serde_json::json;

use super::{
    InstalledProviderArtifact, InstalledProviderArtifactsDocument, RuntimeProviderArtifacts,
    SCHEMA_ID, SCHEMA_VERSION, generation, runtime_source_index_provider_projection,
};

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
                    "requestSchemaId": "agent.semantic-protocols.provider-language-projection-batch-request",
                    "responseSchemaId": "agent.semantic-protocols.provider-language-projection-batch-response"
                }]
            },
            "routes": [{
                "schemaId": "agent.semantic-protocols.provider-route",
                "schemaVersion": "1",
                "routeId": "rust.search.owner",
                "operation": "search.owner",
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
                    "schemaId": "agent.semantic-protocols.search-packet",
                    "mediaType": "application/json"
                },
                "failureSchemaIds": ["agent.semantic-protocols.route-failure"],
                "cache": {
                    "authority": "asp-server",
                    "scope": "workspace",
                    "keySlots": []
                },
                "telemetry": {
                    "spanName": "asp.route.search.owner",
                    "attributeSlots": []
                }
            }]
        }),
    };
    let register =
        agent_semantic_client_db::runtime_provider_register::RuntimeProviderRegister::from_seed(
            vec![registration],
        )
        .expect("live register");
    let providers = vec![InstalledProviderArtifact {
        language_id: "rust".to_owned(),
        provider_id: "asp-rust".to_owned(),
        materialized_path: "/runtime/artifacts/asp-rust".to_owned(),
        artifact_digest: "blake3-256:artifact".to_owned(),
        artifact_metadata_digest: "blake3-256:metadata".to_owned(),
        execution_command_digest: "sha256:command".to_owned(),
    }];
    let artifacts = RuntimeProviderArtifacts {
        document: Arc::new(InstalledProviderArtifactsDocument {
            schema_id: SCHEMA_ID.to_owned(),
            schema_version: SCHEMA_VERSION.to_owned(),
            generation: generation(&providers).expect("generation"),
            providers,
        }),
    };

    let (projection, generation) = runtime_source_index_provider_projection(&artifacts, &register)
        .expect("Runtime source-index provider projection");
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
    assert_eq!(generation, artifacts.document.generation);

    let launch = artifacts
        .runtime_launch(Path::new("/workspace"), "rust", &register)
        .expect("Runtime provider launch");
    assert_eq!(
        launch.spec.env["ASP_PROVIDER_RUNTIME_CONTRACT_DIGEST"],
        launch.expected_receipt.contract_digest
    );
}
