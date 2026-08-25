#[tokio::test]
async fn async_snapshot_reads_each_owner_once_and_preserves_canonical_order() {
    static FIXTURE_ID: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let root = std::env::temp_dir().join(format!(
        "asp-async-source-snapshot-{}-{}",
        std::process::id(),
        FIXTURE_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    ));
    tokio::fs::create_dir_all(root.join("src"))
        .await
        .expect("create async snapshot fixture");
    tokio::fs::write(root.join("src/z.rs"), b"pub fn z() {}\n")
        .await
        .expect("write z owner");
    tokio::fs::write(root.join("src/a.rs"), b"pub fn a() {}\n")
        .await
        .expect("write a owner");
    tokio::fs::write(root.join("Cargo.toml"), b"[package]\nname = \"fixture\"\n")
        .await
        .expect("write auxiliary config");
    let files = ["src/z.rs", "src/a.rs"]
        .into_iter()
        .map(
            |path| agent_semantic_client_db::ClientDbSourceIndexScopeFile {
                path: path.into(),
                language_id: agent_semantic_client_core::LanguageId::from("rust"),
                provider_id: agent_semantic_client_core::ProviderId::from("asp-rust"),
                projection_coverage:
                    agent_semantic_client_db::ClientDbSourceIndexProjectionCoverage::NotDeclared,
                selector_receipts: Vec::new(),
                relations: Vec::new(),
            },
        )
        .collect::<Vec<_>>();
    let registry = agent_semantic_client_core::RuntimeProviderProjectionEvidence {
        fingerprint: "async-snapshot-registry".to_owned(),
        scope_dirs: std::collections::BTreeSet::new(),
    };
    let projection = agent_semantic_client_core::RuntimeProviderProjection {
        authority_ref: "async-snapshot-test".to_owned(),
        providers: vec![agent_semantic_client_core::RuntimeProvider {
            registration_digest: "blake3-256:provider".to_owned(),
            namespace: "agent.semantic-protocols.languages.rust".to_owned(),
            language_id: agent_semantic_client_core::LanguageId::from("rust"),
            provider_id: agent_semantic_client_core::ProviderId::from("asp-rust"),
            binary: "/fixture/asp-rust".to_owned(),
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
            runtime_operations: Vec::new(),
        }],
    };

    let (_, workspace, source, blobs, auxiliary_owners) =
        super::source_index_snapshot_from_files_async(&root, &files, &registry, &projection)
            .await
            .expect("build async source snapshot");
    assert_eq!(source.root_digest, workspace.root_digest());
    assert_eq!(source.leaf_count, 3);
    assert_eq!(auxiliary_owners["asp-rust"].len(), 1);
    assert_eq!(auxiliary_owners["asp-rust"][0].owner_path, "Cargo.toml");
    assert_eq!(
        blobs
            .iter()
            .map(|(path, _)| path.to_owned())
            .collect::<Vec<_>>(),
        vec!["src/a.rs".to_owned(), "src/z.rs".to_owned()]
    );
    tokio::fs::remove_dir_all(root)
        .await
        .expect("remove async snapshot fixture");
}
