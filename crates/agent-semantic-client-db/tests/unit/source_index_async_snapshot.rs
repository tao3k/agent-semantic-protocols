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
        vec![
            "Cargo.toml".to_owned(),
            "src/a.rs".to_owned(),
            "src/z.rs".to_owned(),
        ]
    );
    tokio::fs::remove_dir_all(root)
        .await
        .expect("remove async snapshot fixture");
}

#[tokio::test]
async fn four_thousand_mixed_owner_snapshot_has_subsecond_cold_p95_and_stable_digest() {
    const OWNER_COUNT: usize = 4096;
    const SAMPLE_COUNT: usize = 5;
    let root = std::env::temp_dir().join(format!(
        "asp-source-snapshot-performance-{}",
        std::process::id()
    ));
    tokio::fs::create_dir_all(&root)
        .await
        .expect("create snapshot performance fixture");
    let mut files = Vec::with_capacity(OWNER_COUNT);
    for owner in 0..OWNER_COUNT {
        let path = format!("owner_{owner:04}.rs");
        // Canonical path order intentionally clusters the largest owners at
        // the front. This catches contiguous-shard load imbalance that an
        // equal-size synthetic corpus cannot expose.
        let repetitions = if owner < 96 { 4_096 } else { 1 };
        let contents =
            format!("pub const OWNER_{owner:04}: usize = {owner};\n").repeat(repetitions);
        tokio::fs::write(root.join(&path), contents)
            .await
            .expect("write snapshot performance owner");
        files.push(agent_semantic_client_db::ClientDbSourceIndexScopeFile {
            path: path.into(),
            language_id: agent_semantic_client_core::LanguageId::from("rust"),
            provider_id: agent_semantic_client_core::ProviderId::from("asp-rust"),
            projection_coverage:
                agent_semantic_client_db::ClientDbSourceIndexProjectionCoverage::NotDeclared,
            selector_receipts: Vec::new(),
            relations: Vec::new(),
        });
    }
    let registry = agent_semantic_client_core::RuntimeProviderProjectionEvidence {
        fingerprint: "snapshot-performance-registry".to_owned(),
        scope_dirs: std::collections::BTreeSet::new(),
    };
    let projection = agent_semantic_client_core::RuntimeProviderProjection {
        authority_ref: "snapshot-performance-test".to_owned(),
        providers: vec![agent_semantic_client_core::RuntimeProvider {
            registration_digest: "blake3-256:provider".to_owned(),
            namespace: "agent.semantic-protocols.languages.rust".to_owned(),
            language_id: agent_semantic_client_core::LanguageId::from("rust"),
            provider_id: agent_semantic_client_core::ProviderId::from("asp-rust"),
            binary: "/fixture/asp-rust".to_owned(),
            package_roots: vec![".".to_owned()],
            config_files: Vec::new(),
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
    let mut samples = Vec::with_capacity(SAMPLE_COUNT);
    let mut expected_root_digest = None;
    for _ in 0..SAMPLE_COUNT {
        let started = std::time::Instant::now();
        let (_, _, source, blobs, _) =
            super::source_index_snapshot_from_files_async(&root, &files, &registry, &projection)
                .await
                .expect("build 4096-owner source snapshot");
        samples.push(started.elapsed());
        assert_eq!(source.leaf_count, OWNER_COUNT);
        assert_eq!(blobs.len(), OWNER_COUNT);
        match expected_root_digest.as_deref() {
            None => expected_root_digest = Some(source.root_digest),
            Some(expected) => assert_eq!(source.root_digest, expected),
        }
    }
    samples.sort_unstable();
    let p95 = samples[(SAMPLE_COUNT * 95).div_ceil(100) - 1];
    eprintln!(
        "source-snapshot mixedColdOwners={OWNER_COUNT} samples={SAMPLE_COUNT} p95Micros={}",
        p95.as_micros()
    );
    assert!(
        p95 < std::time::Duration::from_secs(1),
        "4096-owner source snapshot cold p95 exceeded 1s: {samples:?}"
    );
    tokio::fs::remove_dir_all(root)
        .await
        .expect("remove snapshot performance fixture");
}
