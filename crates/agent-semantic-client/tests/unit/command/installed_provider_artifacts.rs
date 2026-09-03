use std::{path::Path, sync::Arc};

use agent_semantic_provider_protocol::ProviderRegistrationDocument;
use serde_json::json;

use super::{
    InstalledProviderArtifact, InstalledProviderArtifactsDocument, RuntimeProviderArtifacts,
    SCHEMA_ID, SCHEMA_VERSION, document_path, generation, integrity_ref,
    load_authoritative_runtime_projection, load_runtime_provider_artifacts,
    publish_current_installed_provider_artifacts, runtime_source_index_provider_projection,
    workspace_required_provider_languages_for_paths,
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
    let incomplete_register =
        agent_semantic_client_db::runtime_provider_register::RuntimeProviderRegister::from_seed(
            vec![registration.clone(), inactive_julia_registration],
        )
        .expect("live register");
    let providers = vec![InstalledProviderArtifact {
        language_id: "rust".to_owned(),
        provider_id: "asp-rust".to_owned(),
        materialized_path: "/runtime/artifacts/asp-rust".to_owned(),
        artifact_digest: format!("blake3-256:{}", "a".repeat(64)),
        artifact_metadata_digest: format!("blake3-256:{}", "b".repeat(64)),
        execution_command_digest: format!("sha256:{}", "c".repeat(64)),
    }];
    let artifacts = RuntimeProviderArtifacts {
        document: Arc::new(InstalledProviderArtifactsDocument {
            schema_id: SCHEMA_ID.to_owned(),
            schema_version: SCHEMA_VERSION.to_owned(),
            generation: generation(&providers).expect("generation"),
            providers,
        }),
        binding: None,
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
    let relocated = RuntimeProviderArtifacts {
        document: Arc::new(relocated_document),
        binding: artifacts.binding.clone(),
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
async fn guarded_install_publication_is_atomic_and_rejects_receipt_drift() {
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
    let registry_digest =
        crate::command::provider_install_registry::provider_install_registry_digest()
            .expect("provider registry digest");
    agent_semantic_artifacts::runtime_artifact_catalog::publish_runtime_provider_catalog(
        &root,
        &format!("blake3-256:{}", "a".repeat(64)),
        &registry_digest,
    )
    .expect("publish provider catalog identity");
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
        "schemaId = \"asp.provider-install-lock.v1\"\nlanguage = \"rust\"\nprovider = \"{}\"\ninstalledPath = \"{}\"\nartifactDigest = \"{}\"\ninstalledEntrypointDigest = \"{}\"\ninstalledEntrypointMetadataDigest = \"{}\"\nexecutionCommandDigest = \"{}\"\n",
        registration.provider_id,
        stable.display(),
        content,
        content,
        metadata,
        execution_command_digest,
    );
    std::fs::write(&receipt_path, &receipt).expect("write provider receipt");
    let first =
        publish_current_installed_provider_artifacts(&root).expect("publish provider snapshot");
    assert!(first.artifact_write);
    assert_eq!(first.changed_leaf_count, 1);
    let first_binding =
        agent_semantic_artifacts::installed_provider_binding::load_installed_provider_binding(
            &root,
        )
        .expect("load installed provider binding")
        .expect("published installed provider binding");
    assert_eq!(first_binding.generation, first.generation());
    assert_eq!(
        first_binding.providers[0].entrypoint_digest,
        integrity_ref(&content)
    );
    let (runtime_projection, runtime_binding) = load_authoritative_runtime_projection(&root)
        .expect("load authoritative Runtime projection");
    assert_eq!(runtime_projection.providers.len(), 1);
    assert_eq!(
        runtime_binding.expect("Runtime binding").generation,
        first.generation()
    );
    let second = publish_current_installed_provider_artifacts(&root)
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

    let next_artifact_digest = format!("blake3-256:{}", "d".repeat(64));
    let changed_artifact_receipt = receipt.replacen(
        &format!("artifactDigest = \"{content}\""),
        &format!("artifactDigest = \"{next_artifact_digest}\""),
        1,
    );
    std::fs::write(&receipt_path, &changed_artifact_receipt)
        .expect("write changed provider artifact identity");
    let changed = publish_current_installed_provider_artifacts(&root)
        .expect("publish changed provider artifact identity");
    assert!(changed.artifact_write);
    assert_eq!(changed.changed_leaf_count, 2);
    assert_ne!(changed.generation(), first.generation());
    let changed_document: InstalledProviderArtifactsDocument = serde_json::from_slice(
        &std::fs::read(document_path(&root)).expect("read changed provider snapshot"),
    )
    .expect("decode changed provider snapshot");
    assert_eq!(
        changed_document.providers[0].artifact_digest,
        next_artifact_digest
    );

    agent_semantic_artifacts::runtime_artifact_catalog::publish_runtime_provider_catalog(
        &root,
        &format!("blake3-256:{}", "a".repeat(64)),
        &format!("blake3-256:{}", "9".repeat(64)),
    )
    .expect("publish drifted catalog generation");
    let drift = load_authoritative_runtime_projection(&root)
        .expect_err("authority drift requires reconciliation");
    assert!(drift.contains("automatic refresh is required"), "{drift}");
    let refreshed = load_runtime_provider_artifacts(&root)
        .await
        .expect("normal authority drift must refresh automatically");
    assert_eq!(refreshed.document.providers.len(), 1);
    assert_eq!(
        agent_semantic_artifacts::runtime_artifact_catalog::load_runtime_provider_catalog_identity(
            &root,
        )
        .expect("load refreshed catalog")
        .expect("refreshed catalog identity")
        .install_registry_digest,
        registry_digest
    );

    std::fs::write(
        &receipt_path,
        receipt.replace(&content, "blake3-256:wrong-content"),
    )
    .expect("write drifted provider receipt");
    let error = publish_current_installed_provider_artifacts(&root)
        .expect_err("receipt content drift must fail closed");
    assert!(error.contains("does not match artifact"), "{error}");

    std::fs::write(&receipt_path, changed_artifact_receipt)
        .expect("restore bound provider receipt");
    std::fs::write(&artifact, b"tampered-provider").expect("tamper provider bytes");
    let tamper = load_authoritative_runtime_projection(&root)
        .expect_err("Runtime must re-admit executable bytes");
    assert!(tamper.contains("content drift"), "{tamper}");

    std::fs::remove_dir_all(root).expect("remove provider publication fixture");
}
