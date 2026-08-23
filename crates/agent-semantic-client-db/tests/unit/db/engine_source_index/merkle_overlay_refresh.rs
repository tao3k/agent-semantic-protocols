use super::{
    CLIENT_DB_SOURCE_INDEX_PROVIDER_ID, CLIENT_DB_SOURCE_INDEX_SCHEMA_ID,
    CLIENT_DB_SOURCE_INDEX_SCHEMA_VERSION, CacheGenerationId, ClientCacheFileHash,
    ClientDbSourceIndexImport, ClientDbSourceIndexImportFile, ClientDbSourceIndexImportRequest,
    ClientDbSourceIndexRefreshRequest, ClientDbSourceIndexSource, LanguageId, ProviderId,
    SemanticSchemaId, SemanticSchemaVersion, build_fixture_source_index_import, temp_root,
};
use std::{fs, path::Path};

fn merkle_selector(
    owner_path: &str,
    symbol: &str,
    source: &[u8],
    tree: &agent_semantic_content_identity::workspace_merkle_v1::WorkspacePathMerkleTreeV1,
) -> agent_semantic_client_db::ClientDbSourceIndexSelector {
    use agent_semantic_content_identity::canonical_item_identity::{
        CanonicalItemIdentity, CanonicalItemSelector,
    };
    use agent_semantic_content_identity::exact_selector_merkle::{
        ExactProjectionModeV1, canonical_content_digest,
    };
    use agent_semantic_content_identity::exact_selector_projection_packet::{
        ExactSelectorProjectionPacketV1Input, ProjectionPacketLanguageIdV1,
        ProjectionPacketOwnerPathV1, ProjectionPacketProviderIdV1,
        ProjectionPacketStructuralSelectorV1, build_exact_selector_projection_packet_v1,
    };

    let structural_selector = format!("rust://{owner_path}#item/function/{symbol}");
    let packet = build_exact_selector_projection_packet_v1(ExactSelectorProjectionPacketV1Input {
        language_id: &ProjectionPacketLanguageIdV1::from("rust"),
        provider_id: &ProjectionPacketProviderIdV1::from("rs-harness"),
        canonical_item_selector: CanonicalItemSelector::new(
            CanonicalItemIdentity::new("rust", "function", symbol),
            structural_selector.clone(),
        ),
        parser_identity_digest: &canonical_content_digest(b"parser", &[b"rs-harness"]),
        query_pack_digest: &canonical_content_digest(b"query-pack", &[b"rust"]),
        owner_path: &ProjectionPacketOwnerPathV1::from(owner_path),
        structural_selector: &ProjectionPacketStructuralSelectorV1::from(
            structural_selector.clone(),
        ),
        projection_mode: ExactProjectionModeV1::Code,
        source_byte_start: 0,
        source_byte_end: source.len() as u64,
        source,
        normalized_parser_facts: br#"{"kind":"fn"}"#,
        projection: source,
    });
    agent_semantic_client_db::ClientDbSourceIndexSelector {
        owner_path: agent_semantic_client_db::ClientDbSourceIndexPath::new(owner_path),
        provider_id: ProviderId::from("rs-harness"),
        selector_id: agent_semantic_client_db::ClientDbSourceIndexSelectorId::from(
            structural_selector,
        ),
        symbol: None,
        kind: None,
        source: ClientDbSourceIndexSource::from(CLIENT_DB_SOURCE_INDEX_PROVIDER_ID),
        query_keys: Vec::new(),
        projection_record: packet
            .enrich_projection_record(tree)
            .expect("enrich Merkle fixture selector"),
        derived_projections: Vec::new(),
    }
}

fn merkle_import(
    project_root: &Path,
    generation_id: &str,
    files: &[(&str, &str, &str)],
) -> (
    ClientDbSourceIndexImport,
    agent_semantic_client_db::ClientDbSourceIndexSourceBlobs,
) {
    let projection_tree =
        agent_semantic_content_identity::workspace_merkle_v1::WorkspacePathMerkleTreeV1::from_file_digests(
            files.iter().map(|(path, _, symbol)| {
                let source = format!("pub fn {symbol}() {{}}\n");
                (
                    (*path).to_string(),
                    agent_semantic_content_identity::exact_selector_merkle::blake3_content_digest_v1(
                        source.as_bytes(),
                    ),
                )
            }),
        )
        .expect("Merkle fixture projection tree");
    let source_blobs = agent_semantic_client_db::ClientDbSourceIndexSourceBlobs::from_normalized(
        files.iter().map(|(path, _, symbol)| {
            (
                agent_semantic_client_db::ClientDbSourceIndexPath::new(*path),
                format!("pub fn {symbol}() {{}}\n").into_bytes(),
            )
        }),
    );
    let import = build_fixture_source_index_import(ClientDbSourceIndexImportRequest {
        source_blobs: source_blobs.clone(),
        generation_id: CacheGenerationId::from(generation_id),
        project_root: project_root.to_path_buf(),
        schema_id: SemanticSchemaId::from(CLIENT_DB_SOURCE_INDEX_SCHEMA_ID),
        schema_version: SemanticSchemaVersion::from(CLIENT_DB_SOURCE_INDEX_SCHEMA_VERSION),
        selector_source: ClientDbSourceIndexSource::from(CLIENT_DB_SOURCE_INDEX_PROVIDER_ID),
        file_hashes: files
            .iter()
            .map(|(path, _, symbol)| {
                let source = format!("pub fn {symbol}() {{}}\n");
                ClientCacheFileHash {
                    path: (*path).to_string(),
                    sha256: format!(
                        "{:x}",
                        <sha2::Sha256 as sha2::Digest>::digest(source.as_bytes())
                    ),
                    byte_len: source.len() as u64,
                    mtime_ms: 1,
                }
            })
            .collect(),
        files: files
            .iter()
            .map(|(path, _, symbol)| ClientDbSourceIndexImportFile {
                relations: Vec::new(),
                relative_path: (*path).to_string(),
                language_id: LanguageId::from("rust"),
                provider_id: ProviderId::from("rs-harness"),
                text: format!("pub fn {symbol}() {{}}\n"),
                selectors: vec![merkle_selector(
                    path,
                    symbol,
                    format!("pub fn {symbol}() {{}}\n").as_bytes(),
                    &projection_tree,
                )],
            })
            .collect(),
    })
    .expect("build Merkle source-index import");
    (import, source_blobs)
}

#[test]
fn merkle_overlay_publishes_an_immutable_generation_for_changed_membership() {
    let project_root = temp_root("db-engine-merkle-overlay-project");
    let fixture = agent_semantic_client_db::fixture::SourceIndexFixture::for_client_dir(
        &project_root.join("client"),
    );
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
    assert_ne!(overlay_report.generation_id, base_report.generation_id);
    assert!(
        overlay_report
            .generation_id
            .as_str()
            .starts_with("source-index-")
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
    let fixture = agent_semantic_client_db::fixture::SourceIndexFixture::for_client_dir(
        &project_root.join("client"),
    );
    let base_snapshot = agent_semantic_content_identity::WorkspaceSnapshot::from_file_hashes([(
        "src/old.rs",
        "a".repeat(64),
    )]);
    let (base_import, base_source_blobs) = merkle_import(
        &project_root,
        "merkle-rename-base",
        &[("src/old.rs", &"a".repeat(64), "old_symbol")],
    );
    let base_report = fixture
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
    assert_ne!(report.generation_id, base_report.generation_id);
    assert!(report.generation_id.as_str().starts_with("source-index-"));
    assert_eq!(report.changed_owner_count, 1);
    assert_eq!(report.removed_owner_count, 1);
    assert_eq!(report.owner_count, 1);

    let _ = fs::remove_dir_all(project_root);
}
