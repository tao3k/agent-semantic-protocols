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
    let files = ["src/z.rs", "src/a.rs"]
        .into_iter()
        .map(
            |path| agent_semantic_client_db::ClientDbSourceIndexScopeFile {
                path: path.into(),
                language_id: agent_semantic_client_core::LanguageId::from("rust"),
                provider_id: agent_semantic_client_core::ProviderId::from("rs-harness"),
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

    let (_, workspace, source, blobs) =
        super::source_index_snapshot_from_files_async(&root, &files, &registry)
            .await
            .expect("build async source snapshot");
    assert_eq!(source.root_digest, workspace.root_digest());
    assert_eq!(source.leaf_count, 2);
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
