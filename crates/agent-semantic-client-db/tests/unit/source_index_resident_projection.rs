// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

fn provider_projection() -> agent_semantic_client_core::RuntimeProviderProjection {
    agent_semantic_client_core::RuntimeProviderProjection {
        authority_ref: "resident-projection-test".to_owned(),
        providers: vec![agent_semantic_client_core::RuntimeProvider {
            registration_digest: format!("blake3-256:{}", "1".repeat(64)),
            execution_artifact_digest: format!("blake3-256:{}", "2".repeat(64)),
            namespace: "agent.semantic-protocols.languages.rust".to_owned(),
            language_id: agent_semantic_client_core::LanguageId::from("rust"),
            provider_id: agent_semantic_client_core::ProviderId::from("asp-rust"),
            binary: "/not-used/asp-rust".to_owned(),
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
                "semanticFacts": false,
                "dependencyTopology": false,
                "dependencyTopologyMetadata": false
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
            runtime_operations: vec![agent_semantic_client_core::RuntimeProviderOperation {
                operation: "projection-batch".to_owned(),
                request_schema_id: "request-v1".to_owned(),
                response_schema_id: "response-v1".to_owned(),
            }],
        }],
    }
}

fn runtime_contract() -> agent_semantic_provider_transport::ProviderRuntimeContractReceipt {
    use agent_semantic_provider_protocol::ProviderSchemaReference;
    use agent_semantic_provider_transport::{
        ProviderRuntimeContractOperation, ProviderRuntimeContractTransport,
    };

    agent_semantic_provider_transport::ProviderRuntimeContractReceipt::new(
        "asp-rust",
        "rust",
        format!("blake3-256:{}", "2".repeat(64)),
        format!("blake3-256:{}", "1".repeat(64)),
        ProviderRuntimeContractTransport::InProcess,
        vec![ProviderRuntimeContractOperation {
            operation: "projection-batch".to_owned(),
            request_schema: ProviderSchemaReference {
                schema_id: "request-v1".to_owned(),
                schema_version: "1".to_owned(),
            },
            response_schema: ProviderSchemaReference {
                schema_id: "response-v1".to_owned(),
                schema_version: "1".to_owned(),
            },
        }],
    )
    .expect("runtime contract")
}

fn owner_snapshot(source: &[u8]) -> crate::runtime_server_workspace::WorkspaceOwnerSnapshot {
    crate::runtime_server_workspace::WorkspaceOwnerSnapshot {
        owner_path: "src/lib.rs".to_owned(),
        authority: Some(agent_semantic_search::ResidentSearchAuthority {
            language_id: agent_semantic_client_core::LanguageId::from("rust"),
            provider_id: agent_semantic_client_core::ProviderId::from("asp-rust"),
        }),
        content_digest: format!("blake3-256:{}", blake3::hash(source).to_hex()),
        native_syntax_diagnostic: None,
        bytes: source.to_vec(),
        selectors: Vec::new(),
    }
}

#[tokio::test]
async fn unchanged_owner_reuses_parser_artifact_without_provider_runtime() {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    use agent_semantic_provider_transport::projection_batch::{
        PROJECTION_BATCH_RESPONSE_SCHEMA_ID, ProviderProjectedOwner,
        ProviderProjectionBatchResponse, ProviderProjectionState,
    };

    let provider_calls = Arc::new(AtomicUsize::new(0));
    let calls = Arc::clone(&provider_calls);
    let authority = agent_semantic_provider_transport::spawn_in_process_provider_runtime_actor(
        1,
        || async { Ok(runtime_contract()) },
        move |operation, mut payload| {
            let calls = Arc::clone(&calls);
            async move {
                assert_eq!(operation, "projection-batch");
                calls.fetch_add(1, Ordering::AcqRel);
                let request: serde_json::Value =
                    serde_json::from_slice(&payload).map_err(|error| error.to_string())?;
                let owner = request["owners"]
                    .as_array()
                    .and_then(|owners| owners.first())
                    .ok_or_else(|| "projection request omitted owner".to_owned())?;
                let response = ProviderProjectionBatchResponse {
                    schema_id: PROJECTION_BATCH_RESPONSE_SCHEMA_ID.to_owned(),
                    schema_version: "1".to_owned(),
                    language_id: request["languageId"]
                        .as_str()
                        .ok_or_else(|| "projection request omitted languageId".to_owned())?
                        .to_owned(),
                    provider_id: request["providerId"]
                        .as_str()
                        .ok_or_else(|| "projection request omitted providerId".to_owned())?
                        .to_owned(),
                    generation_root_digest: request["generationRootDigest"]
                        .as_str()
                        .ok_or_else(|| {
                            "projection request omitted generationRootDigest".to_owned()
                        })?
                        .to_owned(),
                    owners: vec![ProviderProjectedOwner {
                        owner_path: owner["ownerPath"]
                            .as_str()
                            .ok_or_else(|| "projection request omitted ownerPath".to_owned())?
                            .to_owned(),
                        source_leaf_digest: owner["sourceLeafDigest"]
                            .as_str()
                            .ok_or_else(|| {
                                "projection request omitted sourceLeafDigest".to_owned()
                            })?
                            .to_owned(),
                        projection_state: ProviderProjectionState::Ready,
                        diagnostic: None,
                        items: Vec::new(),
                        relations: Vec::new(),
                    }],
                };
                payload = serde_json::to_vec(&response)
                    .map_err(|error| error.to_string())?
                    .into();
                Ok(payload)
            }
        },
    );
    let mut runtime = authority.client();
    runtime.wait_ready().await.expect("resident provider Ready");

    let project_root = std::env::temp_dir().join(format!(
        "asp-resident-projection-content-reuse-{}",
        std::process::id()
    ));
    let artifact_root = tempfile::tempdir().expect("parser artifact root");
    let source = b"pub fn resident() {}\n";
    let first = super::prepare_runtime_server_resident_owner_projections_async(
        Some(runtime),
        project_root.clone(),
        "workspace-resident-projection".to_owned(),
        vec![owner_snapshot(source)],
        Vec::new(),
        provider_projection(),
        artifact_root.path().to_path_buf(),
    )
    .await
    .expect("first projection publishes parser artifact");
    assert_eq!(first.len(), 1);
    assert_eq!(provider_calls.load(Ordering::Acquire), 1);
    authority
        .shutdown()
        .await
        .expect("shutdown provider runtime");

    let reused = super::prepare_runtime_server_resident_owner_projections_async(
        None,
        project_root.clone(),
        "workspace-resident-projection-rebound".to_owned(),
        vec![owner_snapshot(source)],
        Vec::new(),
        provider_projection(),
        artifact_root.path().to_path_buf(),
    )
    .await
    .expect("unchanged content reuses parser artifact without provider");
    assert_eq!(reused, first);
    assert_eq!(provider_calls.load(Ordering::Acquire), 1);

    let changed = b"pub fn changed() {}\n";
    assert_eq!(
        super::prepare_runtime_server_resident_owner_projections_async(
            None,
            project_root,
            "workspace-resident-projection-changed".to_owned(),
            vec![owner_snapshot(changed)],
            Vec::new(),
            provider_projection(),
            artifact_root.path().to_path_buf(),
        )
        .await
        .expect_err("changed content must not reuse parser artifact"),
        "state=cache-miss reasonKind=provider-parser-runtime-required"
    );
}

#[tokio::test]
async fn resident_projection_does_not_reopen_deleted_source_or_auxiliary_files() {
    let project_root = std::env::temp_dir().join(format!(
        "asp-resident-projection-missing-worktree-{}",
        std::process::id()
    ));
    let artifact_root = tempfile::tempdir().expect("parser artifact root");
    assert!(!project_root.exists());
    let source = b"pub fn resident() {}\n".to_vec();
    let auxiliary = b"[package]\nname = \"resident\"\n".to_vec();
    let authority = agent_semantic_search::ResidentSearchAuthority {
        language_id: agent_semantic_client_core::LanguageId::from("rust"),
        provider_id: agent_semantic_client_core::ProviderId::from("asp-rust"),
    };
    let result = super::prepare_runtime_server_resident_owner_projections_async(
        None,
        project_root,
        "workspace-resident-projection".to_owned(),
        vec![crate::runtime_server_workspace::WorkspaceOwnerSnapshot {
            owner_path: "src/lib.rs".to_owned(),
            authority: Some(authority),
            content_digest: format!("blake3-256:{}", blake3::hash(&source).to_hex()),
            native_syntax_diagnostic: None,
            bytes: source,
            selectors: Vec::new(),
        }],
        vec![
            crate::runtime_server_workspace::WorkspaceAuxiliaryOwnerSnapshot {
                owner_path: "Cargo.toml".to_owned(),
                content_digest: format!("blake3-256:{}", blake3::hash(&auxiliary).to_hex()),
                bytes: auxiliary,
            },
        ],
        provider_projection(),
        artifact_root.path().to_path_buf(),
    )
    .await;
    assert_eq!(
        result.expect_err("missing parser artifact needs resident provider"),
        "state=cache-miss reasonKind=provider-parser-runtime-required"
    );
}

#[tokio::test]
async fn resident_projection_rejects_corrupt_auxiliary_identity_without_filesystem_fallback() {
    let source = b"pub fn resident() {}\n".to_vec();
    let authority = agent_semantic_search::ResidentSearchAuthority {
        language_id: agent_semantic_client_core::LanguageId::from("rust"),
        provider_id: agent_semantic_client_core::ProviderId::from("asp-rust"),
    };
    let error = super::prepare_runtime_server_resident_owner_projections_async(
        None,
        std::env::temp_dir().join("asp-resident-projection-corrupt"),
        "workspace-resident-projection".to_owned(),
        vec![crate::runtime_server_workspace::WorkspaceOwnerSnapshot {
            owner_path: "src/lib.rs".to_owned(),
            authority: Some(authority),
            content_digest: format!("blake3-256:{}", blake3::hash(&source).to_hex()),
            native_syntax_diagnostic: None,
            bytes: source,
            selectors: Vec::new(),
        }],
        vec![
            crate::runtime_server_workspace::WorkspaceAuxiliaryOwnerSnapshot {
                owner_path: "Cargo.toml".to_owned(),
                content_digest: format!("blake3-256:{}", "0".repeat(64)),
                bytes: b"changed".to_vec(),
            },
        ],
        provider_projection(),
        std::env::temp_dir().join("asp-resident-projection-artifacts"),
    )
    .await
    .expect_err("corrupt auxiliary identity");
    assert!(error.contains("resident auxiliary content digest drift"));
}
