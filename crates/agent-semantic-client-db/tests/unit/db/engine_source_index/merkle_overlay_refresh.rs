use super::{
    CLIENT_DB_SOURCE_INDEX_PROVIDER_ID, CLIENT_DB_SOURCE_INDEX_SCHEMA_ID,
    CLIENT_DB_SOURCE_INDEX_SCHEMA_VERSION, CacheGenerationId, ClientCacheFileHash, ClientDbEngine,
    ClientDbSourceIndexImport, ClientDbSourceIndexImportFile, ClientDbSourceIndexImportRequest,
    ClientDbSourceIndexRefreshRequest, ClientDbSourceIndexSource, LanguageId, ProviderId,
    SemanticSchemaId, SemanticSchemaVersion, build_source_index_import, temp_root,
};
use std::{fs, path::Path};

fn merkle_import(
    project_root: &Path,
    generation_id: &str,
    files: &[(&str, &str, &str)],
) -> ClientDbSourceIndexImport {
    build_source_index_import(ClientDbSourceIndexImportRequest {
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
    .expect("build Merkle source-index import")
}

#[test]
fn merkle_overlay_reuses_active_generation_and_applies_only_changed_membership() {
    let client_dir = temp_root("db-engine-merkle-overlay-client");
    let project_root = temp_root("db-engine-merkle-overlay-project");
    let base_snapshot = agent_semantic_content_identity::WorkspaceSnapshot::from_file_hashes([
        ("src/a.rs", "a".repeat(64)),
        ("src/b.rs", "b".repeat(64)),
    ]);
    let base_evidence = base_snapshot.evidence(
        agent_semantic_content_identity::SourceSnapshotKind::Filesystem,
        "d".repeat(64),
    );
    let base_import = merkle_import(
        &project_root,
        "merkle-generation-base",
        &[
            ("src/a.rs", &"a".repeat(64), "merkle_a"),
            ("src/b.rs", &"b".repeat(64), "merkle_b"),
        ],
    );
    let base_report = ClientDbEngine::refresh_source_index_import_from_client_dir(
        &client_dir,
        ClientDbSourceIndexRefreshRequest {
            import: base_import,
            file_count: 2,
            source_snapshot: base_evidence,
            membership_change_set:
                agent_semantic_client_db::ClientDbSourceIndexMembershipChangeSet::FullSnapshot,
        },
    )
    .expect("publish Merkle base snapshot");
    assert_eq!(base_report.changed_owner_count, 2);

    let overlay_snapshot =
        base_snapshot.with_overlay_delta([("src/a.rs", "c".repeat(64))], ["src/b.rs"]);
    let overlay_evidence = overlay_snapshot.evidence(
        agent_semantic_content_identity::SourceSnapshotKind::Filesystem,
        "d".repeat(64),
    );
    let overlay_import = merkle_import(
        &project_root,
        "merkle-generation-next",
        &[("src/a.rs", &"c".repeat(64), "merkle_a_changed")],
    );
    let overlay_report = ClientDbEngine::refresh_source_index_import_from_client_dir(
        &client_dir,
        ClientDbSourceIndexRefreshRequest {
            import: overlay_import.clone(),
            file_count: 1,
            source_snapshot: overlay_evidence,
            membership_change_set:
                agent_semantic_client_db::ClientDbSourceIndexMembershipChangeSet::MerkleOverlay {
                    changed_owner_paths: vec![
                        agent_semantic_client_db::ClientDbSourceIndexPath::new("src/a.rs"),
                    ],
                    removed_owner_paths: vec![
                        agent_semantic_client_db::ClientDbSourceIndexPath::new("src/b.rs"),
                    ],
                },
        },
    )
    .expect("apply Merkle owner delta");
    assert_eq!(
        overlay_report.generation_id.as_str(),
        "merkle-generation-base"
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
    let error = ClientDbEngine::refresh_source_index_import_from_client_dir(
        &client_dir,
        ClientDbSourceIndexRefreshRequest {
            import: overlay_import,
            file_count: 1,
            source_snapshot: invalid_evidence,
            membership_change_set:
                agent_semantic_client_db::ClientDbSourceIndexMembershipChangeSet::MerkleOverlay {
                    changed_owner_paths: vec![
                        agent_semantic_client_db::ClientDbSourceIndexPath::new("src/a.rs"),
                    ],
                    removed_owner_paths: Vec::new(),
                },
        },
    )
    .expect_err("mismatched overlay base root must fail closed");
    assert!(error.contains("base root mismatch"), "error={error}");

    let _ = fs::remove_dir_all(client_dir);
    let _ = fs::remove_dir_all(project_root);
}

#[test]
fn merkle_overlay_models_rename_as_one_added_and_one_removed_leaf() {
    let client_dir = temp_root("db-engine-merkle-rename-client");
    let project_root = temp_root("db-engine-merkle-rename-project");
    let base_snapshot = agent_semantic_content_identity::WorkspaceSnapshot::from_file_hashes([(
        "src/old.rs",
        "a".repeat(64),
    )]);
    let base_import = merkle_import(
        &project_root,
        "merkle-rename-base",
        &[("src/old.rs", &"a".repeat(64), "old_symbol")],
    );
    ClientDbEngine::refresh_source_index_import_from_client_dir(
        &client_dir,
        ClientDbSourceIndexRefreshRequest {
            import: base_import,
            file_count: 1,
            source_snapshot: base_snapshot.evidence(
                agent_semantic_content_identity::SourceSnapshotKind::Filesystem,
                "d".repeat(64),
            ),
            membership_change_set:
                agent_semantic_client_db::ClientDbSourceIndexMembershipChangeSet::FullSnapshot,
        },
    )
    .expect("publish rename base snapshot");

    let renamed_snapshot =
        base_snapshot.with_overlay_delta([("src/new.rs", "b".repeat(64))], ["src/old.rs"]);
    let report = ClientDbEngine::refresh_source_index_import_from_client_dir(
        &client_dir,
        ClientDbSourceIndexRefreshRequest {
            import: merkle_import(
                &project_root,
                "merkle-rename-next",
                &[("src/new.rs", &"b".repeat(64), "new_symbol")],
            ),
            file_count: 1,
            source_snapshot: renamed_snapshot.evidence(
                agent_semantic_content_identity::SourceSnapshotKind::Filesystem,
                "d".repeat(64),
            ),
            membership_change_set:
                agent_semantic_client_db::ClientDbSourceIndexMembershipChangeSet::MerkleOverlay {
                    changed_owner_paths: vec![
                        agent_semantic_client_db::ClientDbSourceIndexPath::new("src/new.rs"),
                    ],
                    removed_owner_paths: vec![
                        agent_semantic_client_db::ClientDbSourceIndexPath::new("src/old.rs"),
                    ],
                },
        },
    )
    .expect("apply Merkle rename delta");
    assert_eq!(report.generation_id.as_str(), "merkle-rename-base");
    assert_eq!(report.changed_owner_count, 1);
    assert_eq!(report.removed_owner_count, 1);
    assert_eq!(report.owner_count, 1);

    let _ = fs::remove_dir_all(client_dir);
    let _ = fs::remove_dir_all(project_root);
}
