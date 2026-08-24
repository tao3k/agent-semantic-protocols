use std::{path::Path, sync::Arc};

use agent_semantic_provider_protocol::ProviderRegistrationDocument;
use serde_json::json;

use super::{
    InstalledProviderArtifact, InstalledProviderArtifactsDocument, RuntimeProviderArtifacts,
    SCHEMA_ID, SCHEMA_VERSION, document_path, generation,
    publish_current_installed_provider_artifacts, runtime_source_index_provider_projection,
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

#[cfg(unix)]
#[test]
fn guarded_install_publication_is_atomic_and_rejects_receipt_drift() {
    let root = std::env::temp_dir().join(format!(
        "asp-installed-provider-publication-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&root);
    let artifact_dir = root
        .join("runtime/artifacts/blake3-256")
        .join("fixture-digest");
    let runtime_bin_dir = root.join("runtime/bin");
    let receipt_dir = agent_semantic_runtime::provider_receipt_dir(&root);
    std::fs::create_dir_all(&artifact_dir).expect("create provider CAS fixture");
    std::fs::create_dir_all(&runtime_bin_dir).expect("create provider runtime bin fixture");
    std::fs::create_dir_all(&receipt_dir).expect("create provider receipt fixture");
    let artifact = artifact_dir.join("asp-rust");
    let stable = runtime_bin_dir.join("asp-rust");
    std::fs::write(&artifact, b"asp-rust-provider").expect("write provider CAS artifact");
    std::os::unix::fs::symlink(&artifact, &stable).expect("publish stable provider entry");
    let content =
        agent_semantic_content_identity::file_content_digest_v1(&stable).expect("content digest");
    let metadata = agent_semantic_content_identity::file_artifact_metadata_digest_v1(&stable)
        .expect("metadata digest");
    let execution_command_digest = agent_semantic_hook::provider_execution_command_digest(
        &[stable.to_string_lossy().into_owned()],
        &content,
    )
    .expect("execution command digest");
    let registration =
        crate::command::provider_install_registry::provider_install_registration("rust")
            .expect("registered Rust provider");
    let receipt_path = receipt_dir.join("rust.lock.toml");
    let receipt = format!(
        "schemaId = \"asp.provider-install-lock.v1\"\nlanguage = \"rust\"\nprovider = \"{}\"\ninstalledPath = \"{}\"\ninstalledEntrypointDigest = \"{}\"\ninstalledEntrypointMetadataDigest = \"{}\"\nexecutionCommandDigest = \"{}\"\n",
        registration.provider_id,
        stable.display(),
        content,
        metadata,
        execution_command_digest,
    );
    std::fs::write(&receipt_path, &receipt).expect("write provider receipt");
    let guard = crate::command::protocol_binary::ProtocolBinaryReconciliationGuard::acquire(&root)
        .expect("acquire provider publication guard");

    let first = publish_current_installed_provider_artifacts(&root, &guard)
        .expect("publish provider snapshot");
    assert!(first.artifact_write);
    assert_eq!(first.changed_leaf_count, 1);
    let second = publish_current_installed_provider_artifacts(&root, &guard)
        .expect("observe current provider snapshot");
    assert!(!second.artifact_write);
    assert_eq!(second.changed_leaf_count, 0);
    let document: InstalledProviderArtifactsDocument = serde_json::from_slice(
        &std::fs::read(document_path(&root)).expect("read provider snapshot"),
    )
    .expect("decode provider snapshot");
    assert_eq!(document.providers.len(), 1);
    assert_eq!(document.providers[0].language_id, "rust");
    assert_eq!(
        Path::new(&document.providers[0].materialized_path),
        artifact
            .canonicalize()
            .expect("canonical provider artifact")
    );

    std::fs::write(
        &receipt_path,
        receipt.replace(&content, "blake3-256:wrong-content"),
    )
    .expect("write drifted provider receipt");
    let error = publish_current_installed_provider_artifacts(&root, &guard)
        .expect_err("receipt content drift must fail closed");
    assert!(error.contains("does not match artifact"), "{error}");

    drop(guard);
    std::fs::remove_dir_all(root).expect("remove provider publication fixture");
}
