use agent_semantic_content_identity::{
    SourceSnapshotKind, WorkspaceSnapshot, hash_blob,
};
use agent_semantic_search_projection::{
    RESIDENT_SEARCH_RESULT_SCHEMA_ID, ResidentSearchHit, ResidentSearchProjectionTier,
    ResidentSearchReadyResult, ResidentSearchReadyState,
};

#[test]
fn ready_result_binds_generation_and_proves_zero_request_time_io() {
    let snapshot = WorkspaceSnapshot::from_file_hashes([("src/lib.rs", hash_blob(b"source").value)])
        .evidence(SourceSnapshotKind::Filesystem, hash_blob(b"asp-rust").value);
    let result = ResidentSearchReadyResult::new(
        "blake3-256:1111111111111111111111111111111111111111111111111111111111111111"
            .to_owned(),
        &snapshot,
        "blake3-256:2222222222222222222222222222222222222222222222222222222222222222"
            .to_owned(),
        vec![ResidentSearchHit {
            owner_path: "src/lib.rs".to_owned(),
            owner_content_digest: hash_blob(b"source").value,
            language_id: Some("rust".to_owned()),
            projection_tier: ResidentSearchProjectionTier::ShallowNavigation,
            line_count: 1,
            query_keys: vec!["src/lib.rs".to_owned(), "lib".to_owned()],
            selector: None,
            score: None,
        }],
    )
    .expect("ready resident result");

    assert_eq!(result.schema_id, RESIDENT_SEARCH_RESULT_SCHEMA_ID);
    assert_eq!(result.state, ResidentSearchReadyState::Ready);
    assert_eq!(result.hits.len(), 1);
    result
        .work_counters
        .validate_zero_io()
        .expect("resident result must prove zero request-time I/O");
}
