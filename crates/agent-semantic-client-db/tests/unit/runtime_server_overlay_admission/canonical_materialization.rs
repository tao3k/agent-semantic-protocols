// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Canonical materialization identity and completeness scenario.

use agent_semantic_client_db::runtime_server_workspace::WorkspaceCanonicalMaterialization;
use agent_semantic_client_db::runtime_server_workspace::WorkspaceOwnerSnapshot;
use sha2::Digest as _;

#[test]
fn canonical_materialization_binds_snapshot_import_and_complete_owner_count() {
    let project_root = super::fixture_root();
    std::fs::create_dir_all(&project_root).expect("create canonical materialization project root");
    let import = agent_semantic_client_db::build_source_index_import(
        agent_semantic_client_db::ClientDbSourceIndexImportRequest {
            source_blobs: agent_semantic_client_db::ClientDbSourceIndexSourceBlobs::from_normalized(
                [(
                    agent_semantic_client_db::ClientDbSourceIndexPath::from("src/lib.rs"),
                    b"source".to_vec(),
                )],
            ),
            generation_id: agent_semantic_client_core::CacheGenerationId::from(
                "canonical-materialization-generation",
            ),
            project_root: project_root.clone(),
            schema_id: agent_semantic_client_core::SemanticSchemaId::from(
                agent_semantic_client_db::CLIENT_DB_SOURCE_INDEX_SCHEMA_ID,
            ),
            schema_version: agent_semantic_client_core::SemanticSchemaVersion::from(
                agent_semantic_client_db::CLIENT_DB_SOURCE_INDEX_SCHEMA_VERSION,
            ),
            selector_source: agent_semantic_client_db::ClientDbSourceIndexSource::from(
                agent_semantic_client_db::CLIENT_DB_SOURCE_INDEX_PROVIDER_ID,
            ),
            file_hashes: vec![agent_semantic_client_core::ClientCacheFileHash {
                path: "src/lib.rs".to_owned(),
                sha256: format!("{:x}", sha2::Sha256::digest(b"source")),
                byte_len: 6,
                mtime_ms: 1,
            }],
            files: vec![agent_semantic_client_db::ClientDbSourceIndexImportFile {
                relations: Vec::new(),
                relative_path: "src/lib.rs".to_owned(),
                language_id: "rust".into(),
                provider_id: "asp-rust".into(),
                text: "source".to_owned(),
                selectors: vec![crate::db_engine_source_index::rust_selector_fixture(
                    "src/lib.rs",
                    "rust://src/lib.rs#item/function/fixture",
                    "fixture",
                    b"source",
                )],
            }],
        },
    )
    .expect("build canonical materialization source-index import");
    let workspace_snapshot = agent_semantic_content_identity::WorkspaceSnapshot::from_file_hashes(
        [("src/lib.rs", blake3::hash(b"source").to_hex().to_string())],
    );
    let source_snapshot = workspace_snapshot.evidence(
        agent_semantic_content_identity::SourceSnapshotKind::Filesystem,
        "blake3-256:1111111111111111111111111111111111111111111111111111111111111111".to_owned(),
    );
    let mut materialization = WorkspaceCanonicalMaterialization::new(
        "workspace-canonical-materialization",
        source_snapshot.clone(),
        &import,
        [1, 0],
        vec![WorkspaceOwnerSnapshot {
            authority: None,
            owner_path: "src/lib.rs".to_owned(),
            content_digest: format!("blake3-256:{}", blake3::hash(b"source").to_hex()),
            bytes: b"source".to_vec(),
            native_syntax_diagnostic: None,
            selectors: vec![
                agent_semantic_client_db::runtime_server_workspace::WorkspaceSelectorSnapshot {
                    selector: "rust://src/lib.rs#item/function/fixture".to_owned(),
                    byte_start: 0,
                    byte_end: b"source".len(),
                    query_keys: Vec::new(),
                    derived_projections: Vec::new(),
                },
            ],
        }],
        Vec::new(),
    )
    .expect("build complete materialization");
    materialization
        .attach_content_search_generation(crate::fixture::content_search_generation_receipt(
            "workspace-canonical-materialization",
            &source_snapshot,
        ))
        .expect("bind complete content-search construction receipt");

    materialization
        .validate_against(
            "workspace-canonical-materialization",
            &source_snapshot,
            &import,
            1,
        )
        .expect("validate matching durable request");
    materialization
        .validate_refresh_request(
            "workspace-canonical-materialization",
            &agent_semantic_client_db::ClientDbSourceIndexRefreshRequest {
                import: import.clone(),
                file_count: 9,
                source_snapshot: source_snapshot.clone(),
            },
        )
        .expect("file leaf count may differ from complete owner count");
    let mut drifted_snapshot = source_snapshot.clone();
    drifted_snapshot.root_digest = "c".repeat(64);
    assert!(
        materialization
            .validate_against(
                "workspace-canonical-materialization",
                &drifted_snapshot,
                &import,
                1,
            )
            .expect_err("snapshot drift must fail")
            .contains("source snapshot drift")
    );
    assert!(
        materialization
            .validate_against(
                "workspace-canonical-materialization",
                &source_snapshot,
                &import,
                2,
            )
            .expect_err("partial owner materialization must fail")
            .contains("is incomplete")
    );
    let mut drifted_owner = materialization.clone();
    drifted_owner.owners[0].bytes = b"different-source".to_vec();
    assert!(
        drifted_owner
            .validate_against(
                "workspace-canonical-materialization",
                &source_snapshot,
                &import,
                1,
            )
            .expect_err("owner byte drift must fail")
            .contains("owner digest drift")
    );

    let generation = materialization
        .into_generation(4)
        .expect("build next immutable generation");
    assert_eq!(generation.active_epoch, 5);
    assert_eq!(generation.root_depth, [1, 0]);
    assert_eq!(generation.owners.len(), 1);
    assert_eq!(
        generation.workspace_snapshot.root_digest(),
        generation.source_snapshot.root_digest
    );
    assert_eq!(
        generation.workspace_generation.root_digest,
        generation.source_snapshot.root_digest
    );
    assert_eq!(generation.workspace_generation.leaf_count, 1);
    assert_eq!(generation.workspace_generation.owner_count, 1);
    let _ = std::fs::remove_dir_all(project_root);
}
