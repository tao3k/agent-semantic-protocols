use super::{
    Instant, LanguageId, ProviderId, SOURCE_INDEX_HASH_REUSE_GATE, commit_fixture_generation,
    refresh_request, run_git, source_index_refresh_test_guard, temp_project_root,
};

#[test]
fn source_index_hash_reuse_ignores_scope_dir_mtime() {
    let root = temp_project_root("source-index-hash-reuse-dir-mtime");
    let project_root = root.join("project");
    let source_dir = project_root.join("src");
    std::fs::create_dir_all(&source_dir).expect("create project src dir");
    let source_path = source_dir.join("source_index_perf.rs");
    std::fs::write(&source_path, "pub fn source_index_perf_fixture() {}\n")
        .expect("write source fixture");
    let files = vec![agent_semantic_client_db::ClientDbSourceIndexScopeFile {
        relations: Vec::new(),
        path: source_path.clone(),
        language_id: LanguageId::from("rust"),
        provider_id: ProviderId::from("asp-rust"),
        projection_coverage:
            agent_semantic_client_db::ClientDbSourceIndexProjectionCoverage::NotDeclared,
        selector_receipts: Vec::new(),
    }];
    let source_blobs =
        agent_semantic_client_db::ClientDbSourceIndexSourceBlobs::from_normalized([(
            agent_semantic_client_db::ClientDbSourceIndexPath::from("src/source_index_perf.rs"),
            std::fs::read(&source_path).expect("read source fixture"),
        )]);

    let first = agent_semantic_client_db::source_index_file_hashes(
        &project_root,
        &files,
        &source_blobs,
        "registry-fingerprint",
        std::iter::empty(),
    )
    .expect("initial source-index file hashes");

    let transient = source_dir.join(".transient-source-index-mtime");
    std::fs::write(&transient, "mtime witness").expect("write transient file");
    std::fs::remove_file(&transient).expect("remove transient file");

    let started_at = Instant::now();
    let second = agent_semantic_client_db::source_index_file_hashes(
        &project_root,
        &files,
        &source_blobs,
        "registry-fingerprint",
        std::iter::empty(),
    )
    .expect("reused source-index file hashes");
    let elapsed = started_at.elapsed();

    assert_eq!(
        second, first,
        "source-index no-op fingerprint must ignore directory mtime churn"
    );
    assert!(
        elapsed < SOURCE_INDEX_HASH_REUSE_GATE,
        "source-index hash reuse should remain millisecond-scale; elapsed={elapsed:?} gate={SOURCE_INDEX_HASH_REUSE_GATE:?}"
    );

    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn source_index_dirty_git_path_forces_content_hash_despite_metadata_collision() {
    let root = temp_project_root("source-index-dirty-git-hash");
    let project_root = root.join("project");
    let source_dir = project_root.join("src");
    std::fs::create_dir_all(&source_dir).expect("create project src dir");
    let source_path = source_dir.join("dirty_hash.rs");
    std::fs::write(&source_path, "pub fn first() {}\n").expect("write source fixture");
    run_git(&project_root, ["init", "--quiet"]);
    run_git(&project_root, ["add", "src/dirty_hash.rs"]);
    run_git(
        &project_root,
        [
            "-c",
            "user.email=source-index@example.invalid",
            "-c",
            "user.name=Source Index",
            "commit",
            "--quiet",
            "-m",
            "initial",
        ],
    );
    let files = vec![agent_semantic_client_db::ClientDbSourceIndexScopeFile {
        relations: Vec::new(),
        path: source_path.clone(),
        language_id: LanguageId::from("rust"),
        provider_id: ProviderId::from("asp-rust"),
        projection_coverage:
            agent_semantic_client_db::ClientDbSourceIndexProjectionCoverage::NotDeclared,
        selector_receipts: Vec::new(),
    }];
    let first_source_blobs =
        agent_semantic_client_db::ClientDbSourceIndexSourceBlobs::from_normalized([(
            agent_semantic_client_db::ClientDbSourceIndexPath::from("src/dirty_hash.rs"),
            std::fs::read(&source_path).expect("read initial source fixture"),
        )]);
    let first = agent_semantic_client_db::source_index_file_hashes(
        &project_root,
        &files,
        &first_source_blobs,
        "registry-fingerprint",
        std::iter::empty(),
    )
    .expect("initial source-index file hashes");

    std::fs::write(&source_path, "pub fn other() {}\n").expect("rewrite source fixture");
    let second_source_blobs =
        agent_semantic_client_db::ClientDbSourceIndexSourceBlobs::from_normalized([(
            agent_semantic_client_db::ClientDbSourceIndexPath::from("src/dirty_hash.rs"),
            std::fs::read(&source_path).expect("read rewritten source fixture"),
        )]);
    let second = agent_semantic_client_db::source_index_file_hashes(
        &project_root,
        &files,
        &second_source_blobs,
        "registry-fingerprint",
        std::iter::empty(),
    )
    .expect("dirty source-index file hashes");

    assert_ne!(
        second[0].sha256, first[0].sha256,
        "a Git-dirty path must bypass metadata-only hash reuse"
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn source_index_refresh_publishes_distinct_immutable_generation_identities() {
    let _test_guard = source_index_refresh_test_guard();
    let root = temp_project_root("source-index-refresh-restore-snapshot-identity");
    let client_dir = root.join("client");
    let fixture =
        agent_semantic_client_db::fixture::SourceIndexFixture::for_client_dir(&client_dir);
    let project_root = root.join("project");
    std::fs::create_dir_all(&client_dir).expect("create client dir");
    std::fs::create_dir_all(project_root.join("src")).expect("create project src dir");

    let first_snapshot = crate::snapshot_fixture::source_snapshot_evidence_for(1);
    let mut first_request = refresh_request(&project_root);
    first_request.import.generation_id =
        agent_semantic_client_db::client_db_source_index_generation_id_for_snapshot(
            &first_snapshot,
        );
    first_request.source_snapshot = first_snapshot.clone();
    let first = commit_fixture_generation(&fixture, first_request)
        .expect("write initial source-index facts");
    assert!(!first.reused_generation);

    let mut changed_request = refresh_request(&project_root);
    let changed_source = b"pub fn source_index_perf_fixture() { changed(); }";
    changed_request.import.file_hashes[0].sha256 = format!(
        "{:x}",
        <sha2::Sha256 as sha2::Digest>::digest(changed_source)
    );
    changed_request.import.file_hashes[0].byte_len = changed_source.len() as u64;
    changed_request.import.selectors[0].source = String::from_utf8(changed_source.to_vec())
        .expect("changed fixture UTF-8")
        .into();
    changed_request.import.selectors[0].projection_record =
        crate::projection_fixture::projection_record(
            crate::projection_fixture::ProjectionFixtureInput {
                language_id: "rust",
                provider_id: "asp-rust",
                owner_path: "src/source_index_perf.rs",
                structural_selector: "rust://src/source_index_perf.rs#item/function/source_index_perf_fixture",
                item_kind: "function",
                item_name: "source_index_perf_fixture",
                source: changed_source,
                source_byte_start: 0,
                source_byte_end: changed_source.len() as u64,
            },
        );
    let changed_snapshot =
        agent_semantic_content_identity::WorkspaceSnapshot::from_file_hashes([(
            "src/source_index_perf.rs",
            blake3::hash(changed_source).to_hex().to_string(),
        )])
        .evidence(
            agent_semantic_content_identity::SourceSnapshotKind::Filesystem,
            first_snapshot.provider_digest.clone(),
        );
    changed_request.import.generation_id =
        agent_semantic_client_db::client_db_source_index_generation_id_for_snapshot(
            &changed_snapshot,
        );
    changed_request.source_snapshot = changed_snapshot;
    let changed = commit_fixture_generation(&fixture, changed_request)
        .expect("publish changed source-index membership");
    assert!(!changed.reused_generation);

    let mut restored_request = refresh_request(&project_root);
    restored_request.import.generation_id =
        agent_semantic_client_db::client_db_source_index_generation_id_for_snapshot(
            &first_snapshot,
        );
    restored_request.source_snapshot = first_snapshot;
    let restored = commit_fixture_generation(&fixture, restored_request)
        .expect("republish historical source-index facts");
    assert_eq!(
        restored.generation_id, first.generation_id,
        "first={:?} changed={:?} restored={:?}",
        first.generation_id, changed.generation_id, restored.generation_id
    );
    assert_ne!(
        changed.generation_id, first.generation_id,
        "different Merkle snapshots must publish distinct immutable generations"
    );
    assert_ne!(
        changed.source_snapshot.root_digest,
        first.source_snapshot.root_digest
    );
    assert_eq!(
        restored.source_snapshot.root_digest,
        first.source_snapshot.root_digest
    );

    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn source_index_refresh_uses_projection_proof_instead_of_legacy_selector_text() {
    let _test_guard = source_index_refresh_test_guard();
    let root = temp_project_root("source-index-selector-only-refresh");
    let client_dir = root.join("client");
    let fixture =
        agent_semantic_client_db::fixture::SourceIndexFixture::for_client_dir(&client_dir);
    let project_root = root.join("project");
    std::fs::create_dir_all(&client_dir).expect("create client dir");
    std::fs::create_dir_all(project_root.join("src")).expect("create project src dir");

    let first = commit_fixture_generation(&fixture, refresh_request(&project_root))
        .expect("write initial source-index facts");

    let mut changed_request = refresh_request(&project_root);
    changed_request.import.selectors[0].source =
        "pub fn source_index_perf_fixture() { changed(); }".into();
    let replay = commit_fixture_generation(&fixture, changed_request)
        .expect("legacy selector text is outside proof authority");
    assert!(replay.reused_generation);
    assert_eq!(replay.generation_id, first.generation_id);

    let _ = std::fs::remove_dir_all(root);
}
