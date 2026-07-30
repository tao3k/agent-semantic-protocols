use super::{
    CLIENT_DB_SOURCE_INDEX_PROVIDER_ID, CLIENT_DB_SOURCE_INDEX_SCHEMA_ID,
    CLIENT_DB_SOURCE_INDEX_SCHEMA_VERSION, CacheGenerationId, ClientCacheFileHash,
    ClientDbSourceIndexImport, ClientDbSourceIndexImportFile, ClientDbSourceIndexImportRequest,
    ClientDbSourceIndexRefreshRequest, ClientDbSourceIndexSource, LanguageId, ProviderId,
    SemanticSchemaId, SemanticSchemaVersion, build_source_index_import, temp_root,
};
use std::{fs, path::Path};

fn merkle_import(
    project_root: &Path,
    generation_id: &str,
    files: &[(&str, &str, &str)],
) -> (
    ClientDbSourceIndexImport,
    agent_semantic_client_db::ClientDbSourceIndexSourceBlobs,
) {
    let import = build_source_index_import(ClientDbSourceIndexImportRequest {
        generation_id: CacheGenerationId::from(generation_id),
        project_root: project_root.to_path_buf(),
        schema_id: SemanticSchemaId::from(CLIENT_DB_SOURCE_INDEX_SCHEMA_ID),
        schema_version: SemanticSchemaVersion::from(CLIENT_DB_SOURCE_INDEX_SCHEMA_VERSION),
        selector_source: ClientDbSourceIndexSource::from(CLIENT_DB_SOURCE_INDEX_PROVIDER_ID),
        file_hashes: files
            .iter()
            .map(|(path, digest, _)| ClientCacheFileHash {
                path: (*path).to_string(),
                sha256: (*digest).to_string(),
                byte_len: 32,
                mtime_ms: 1,
            })
            .collect(),
        files: files
            .iter()
            .map(|(path, _, symbol)| ClientDbSourceIndexImportFile {
                relative_path: (*path).to_string(),
                language_id: LanguageId::from("rust"),
                provider_id: ProviderId::from("rs-harness"),
                text: format!("pub fn {symbol}() {{}}\n"),
                selectors: Vec::new(),
            })
            .collect(),
    })
    .expect("build Merkle source-index import");
    let source_blobs = agent_semantic_client_db::ClientDbSourceIndexSourceBlobs::from_normalized(
        files.iter().map(|(path, _, symbol)| {
            (
                agent_semantic_client_db::ClientDbSourceIndexPath::new(*path),
                format!("pub fn {symbol}() {{}}\n").into_bytes(),
            )
        }),
    );
    (import, source_blobs)
}

#[test]
fn merkle_overlay_publishes_an_immutable_generation_for_changed_membership() {
    let project_root = temp_root("db-engine-merkle-overlay-project");
    let fixture = agent_semantic_client_db::fixture::SourceIndexFixture::default();
    let base_snapshot = agent_semantic_content_identity::WorkspaceSnapshot::from_file_hashes([
        ("src/a.rs", "a".repeat(64)),
        ("src/b.rs", "b".repeat(64)),
    ]);
    let base_evidence = base_snapshot.evidence(
        agent_semantic_content_identity::SourceSnapshotKind::Filesystem,
        "d".repeat(64),
    );
    let (base_import, base_source_blobs) = merkle_import(
        &project_root,
        "merkle-generation-base",
        &[
            ("src/a.rs", &"a".repeat(64), "merkle_a"),
            ("src/b.rs", &"b".repeat(64), "merkle_b"),
        ],
    );
    let base_report = fixture
        .commit_source_index_generation(
            ClientDbSourceIndexRefreshRequest {
                import: base_import,
                file_count: 2,
                source_snapshot: base_evidence,
            },
            &base_source_blobs,
        )
        .expect("publish Merkle base snapshot");
    assert_eq!(base_report.changed_owner_count, 2);

    let overlay_snapshot =
        base_snapshot.with_overlay_delta([("src/a.rs", "c".repeat(64))], ["src/b.rs"]);
    let overlay_evidence = overlay_snapshot.evidence(
        agent_semantic_content_identity::SourceSnapshotKind::Filesystem,
        "d".repeat(64),
    );
    let (overlay_import, overlay_source_blobs) = merkle_import(
        &project_root,
        "merkle-generation-next",
        &[("src/a.rs", &"c".repeat(64), "merkle_a_changed")],
    );
    let overlay_report = fixture
        .commit_source_index_generation(
            ClientDbSourceIndexRefreshRequest {
                import: overlay_import.clone(),
                file_count: 1,
                source_snapshot: overlay_evidence,
            },
            &overlay_source_blobs,
        )
        .expect("apply Merkle owner delta");
    assert_eq!(
        overlay_report.generation_id.as_str(),
        "merkle-generation-next"
    );
    assert_eq!(overlay_report.changed_owner_count, 1);
    assert_eq!(overlay_report.removed_owner_count, 1);
    assert_eq!(overlay_report.owner_count, 1);

    let invalid_evidence = overlay_snapshot
        .overlay_evidence(
            agent_semantic_content_identity::SourceSnapshotKind::Filesystem,
            "d".repeat(64),
            "f".repeat(64),
            ["src/a.rs"],
            std::iter::empty::<&str>(),
        )
        .expect("construct mismatched overlay evidence");
    let receipt = fixture
        .commit_source_index_generation(
            ClientDbSourceIndexRefreshRequest {
                import: overlay_import,
                file_count: 1,
                source_snapshot: invalid_evidence,
            },
            &overlay_source_blobs,
        )
        .expect("resident writer must derive membership instead of trusting client overlay");
    let forged_base_root = "f".repeat(64);
    assert_ne!(
        receipt.source_snapshot.base_root_digest.as_deref(),
        Some(forged_base_root.as_str())
    );

    let _ = fs::remove_dir_all(project_root);
}

#[test]
fn merkle_overlay_models_rename_as_one_added_and_one_removed_leaf() {
    let project_root = temp_root("db-engine-merkle-rename-project");
    let fixture = agent_semantic_client_db::fixture::SourceIndexFixture::default();
    let base_snapshot = agent_semantic_content_identity::WorkspaceSnapshot::from_file_hashes([(
        "src/old.rs",
        "a".repeat(64),
    )]);
    let (base_import, base_source_blobs) = merkle_import(
        &project_root,
        "merkle-rename-base",
        &[("src/old.rs", &"a".repeat(64), "old_symbol")],
    );
    fixture
        .commit_source_index_generation(
            ClientDbSourceIndexRefreshRequest {
                import: base_import,
                file_count: 1,
                source_snapshot: base_snapshot.evidence(
                    agent_semantic_content_identity::SourceSnapshotKind::Filesystem,
                    "d".repeat(64),
                ),
            },
            &base_source_blobs,
        )
        .expect("publish rename base snapshot");

    let renamed_snapshot =
        base_snapshot.with_overlay_delta([("src/new.rs", "b".repeat(64))], ["src/old.rs"]);
    let (renamed_import, renamed_source_blobs) = merkle_import(
        &project_root,
        "merkle-rename-next",
        &[("src/new.rs", &"b".repeat(64), "new_symbol")],
    );
    let report = fixture
        .commit_source_index_generation(
            ClientDbSourceIndexRefreshRequest {
                import: renamed_import,
                file_count: 1,
                source_snapshot: renamed_snapshot.evidence(
                    agent_semantic_content_identity::SourceSnapshotKind::Filesystem,
                    "d".repeat(64),
                ),
            },
            &renamed_source_blobs,
        )
        .expect("apply Merkle rename delta");
    assert_eq!(report.generation_id.as_str(), "merkle-rename-next");
    assert_eq!(report.changed_owner_count, 1);
    assert_eq!(report.removed_owner_count, 1);
    assert_eq!(report.owner_count, 1);

    let _ = fs::remove_dir_all(project_root);
}
