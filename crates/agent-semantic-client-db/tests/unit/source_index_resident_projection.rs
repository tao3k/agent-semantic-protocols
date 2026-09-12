// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

fn provider_projection() -> agent_semantic_client_core::RuntimeProviderProjection {
    agent_semantic_client_core::RuntimeProviderProjection {
        authority_ref: "resident-projection-test".to_owned(),
        providers: vec![agent_semantic_client_core::RuntimeProvider {
            registration_digest: format!("blake3-256:{}", "1".repeat(64)),
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
