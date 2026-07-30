use agent_semantic_client_core::{
    CacheGenerationId, ClientCacheFileHash, LanguageId, ProviderId, SemanticSchemaId,
    SemanticSchemaVersion,
};
use agent_semantic_client_db::{
    CLIENT_DB_SOURCE_INDEX_PROVIDER_ID, CLIENT_DB_SOURCE_INDEX_SCHEMA_ID,
    CLIENT_DB_SOURCE_INDEX_SCHEMA_VERSION, ClientDbSourceIndexImport, ClientDbSourceIndexOwner,
    ClientDbSourceIndexPath, ClientDbSourceIndexQueryKey, ClientDbSourceIndexRefreshRequest,
    ClientDbSourceIndexSelector, ClientDbSourceIndexSource, ClientDbSourceIndexSourceBlobs,
};

fn fixture_root() -> std::path::PathBuf {
    std::env::temp_dir().join(format!(
        "asp-turso-materialization-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system clock after Unix epoch")
            .as_nanos()
    ))
}

fn generation_fixture(
    project_root: &std::path::Path,
) -> (
    ClientDbSourceIndexRefreshRequest,
    ClientDbSourceIndexSourceBlobs,
) {
    let owner_path = "src/materialized.rs";
    let selector = "rust://src/materialized.rs#item/function/materialized";
    let source = b"pub fn materialized() {}\n";
    let import = ClientDbSourceIndexImport {
        generation_id: CacheGenerationId::from("materialized-generation"),
        project_root: project_root.to_path_buf(),
        schema_id: SemanticSchemaId::from(CLIENT_DB_SOURCE_INDEX_SCHEMA_ID),
        schema_version: SemanticSchemaVersion::from(CLIENT_DB_SOURCE_INDEX_SCHEMA_VERSION),
        file_hashes: vec![ClientCacheFileHash {
            path: owner_path.to_owned(),
            sha256: "a".repeat(64),
            byte_len: source.len() as u64,
            mtime_ms: 1,
        }],
        owners: vec![ClientDbSourceIndexOwner {
            owner_path: ClientDbSourceIndexPath::new(owner_path),
            language_id: Some(LanguageId::from("rust")),
            provider_id: Some(ProviderId::from("rs-harness")),
            source_kind: ClientDbSourceIndexSource::from(CLIENT_DB_SOURCE_INDEX_PROVIDER_ID),
            line_count: Some(1),
            query_keys: vec![ClientDbSourceIndexQueryKey::from("materialized")],
        }],
        selectors: vec![ClientDbSourceIndexSelector {
            owner_path: ClientDbSourceIndexPath::new(owner_path),
            selector_id: selector.into(),
            symbol: Some("materialized".into()),
            kind: Some("function".into()),
            source: ClientDbSourceIndexSource::from(CLIENT_DB_SOURCE_INDEX_PROVIDER_ID),
            query_keys: vec![ClientDbSourceIndexQueryKey::from("materialized")],
            materialization_proof: crate::materialization_fixture::materialization_proof(
                crate::materialization_fixture::MaterializationFixtureInput {
                    language_id: "rust",
                    provider_id: "rs-harness",
                    owner_path,
                    structural_selector: selector,
                    item_kind: "function",
                    item_name: "materialized",
                    source,
                    source_byte_start: 0,
                    source_byte_end: source.len() as u64,
                },
            ),
        }],
    };
    let source_blobs =
        crate::materialization_fixture::source_blobs_fixture([(owner_path, source.as_slice())]);
    (
        ClientDbSourceIndexRefreshRequest {
            import,
            file_count: 1,
            source_snapshot: crate::snapshot_fixture::source_snapshot_evidence(),
        },
        source_blobs,
    )
}

#[test]
fn turso_generation_materialization_is_reusable_and_drift_is_fail_closed() {
    let root = fixture_root();
    let client_dir = root.join("client");
    let project_root = root.join("project");
    let workspace_identity = project_root.display().to_string();
    let (request, source_blobs) = generation_fixture(&project_root);

    let first = agent_semantic_client_db::fixture::commit_source_index_generation_from_fixture_dir(
        &client_dir,
        request.clone(),
        &source_blobs,
    )
    .expect("commit canonical generation and materialization");
    assert!(!first.reused_generation);

    let active =
        agent_semantic_client_db::fixture::load_active_workspace_generation_materialization_from_fixture_dir(
            &client_dir,
            &workspace_identity,
        )
        .expect("load active materialization")
        .expect("active materialization must exist");
    assert_eq!(
        active.source_snapshot.root_digest,
        first.source_snapshot.root_digest
    );
    assert_eq!(
        active.workspace_snapshot.root_digest(),
        active.source_snapshot.root_digest
    );
    assert_eq!(
        active.workspace_generation.root_digest,
        active.source_snapshot.root_digest
    );
    assert_eq!(active.workspace_generation.leaf_count, 1);
    assert_eq!(active.workspace_generation.owner_count, 1);
    assert_eq!(active.root_depth, [1, 0]);
    assert_eq!(active.owners.len(), 1);

    let reused =
        agent_semantic_client_db::fixture::commit_source_index_generation_from_fixture_dir(
            &client_dir,
            request.clone(),
            &source_blobs,
        )
        .expect("reuse identical canonical materialization");
    assert!(reused.reused_generation);
    assert_eq!(reused.generation_id, first.generation_id);

    let mut drifted = active.clone();
    drifted.owners[0].bytes = b"pub fn forged_value() {}\n".to_vec();
    drifted.owners[0].content_digest = format!(
        "blake3-256:{}",
        blake3::hash(&drifted.owners[0].bytes).to_hex()
    );
    let error =
        agent_semantic_client_db::fixture::commit_source_index_generation_with_materialization_from_fixture_dir(
            &client_dir,
            request,
            drifted,
        )
        .expect_err("forged exact owner bytes must fail closed");
    assert!(
        error.contains("proof source drift") || error.contains("proof projection drift"),
        "unexpected materialization drift error: {error}"
    );

    let after_failure =
        agent_semantic_client_db::fixture::load_active_workspace_generation_materialization_from_fixture_dir(
            &client_dir,
            &workspace_identity,
        )
        .expect("reload active materialization")
        .expect("active materialization must remain visible");
    assert_eq!(after_failure, active);

    let _ = std::fs::remove_dir_all(root);
}
