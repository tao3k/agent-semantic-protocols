// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-only

use super::ClientDbEngine;
use super::ClientDbSourceIndexLookupState;
use super::Duration;
use super::Instant;
use super::LanguageId;
use super::commit_fixture_generation;
use super::large_refresh_request;
use super::refresh_request;
use super::source_index_refresh_test_guard;
use super::temp_project_root;

#[tokio::test(flavor = "current_thread")]
async fn source_index_incremental_refresh_prunes_removed_owner_and_postings() {
    let _test_guard = source_index_refresh_test_guard();
    let root = temp_project_root("source-index-incremental-prune");
    let client_dir = root.join("client");
    let fixture =
        agent_semantic_client_db::fixture::SourceIndexFixture::for_client_dir(&client_dir);
    let project_root = root.join("project");
    std::fs::create_dir_all(&client_dir).expect("create client dir");
    std::fs::create_dir_all(project_root.join("src")).expect("create project src dir");

    let mut initial_request = large_refresh_request(&project_root, 2);
    let initial_workspace_snapshot =
        agent_semantic_content_identity::WorkspaceSnapshot::from_file_hashes(
            initial_request
                .import
                .file_hashes
                .iter()
                .map(|file_hash| (file_hash.path.clone(), file_hash.sha256.clone())),
        );
    let initial_snapshot = initial_workspace_snapshot.evidence(
        agent_semantic_content_identity::SourceSnapshotKind::Filesystem,
        "d".repeat(64),
    );
    let survivor_file_hash = initial_request
        .import
        .file_hashes
        .iter()
        .find(|file_hash| file_hash.path == "src/generated/owner_0.rs")
        .expect("owner_0 is the removal-only survivor")
        .clone();
    initial_request.import.generation_id =
        agent_semantic_client_db::client_db_source_index_generation_id_for_snapshot(
            &initial_snapshot,
        );
    initial_request.source_snapshot = initial_snapshot;
    commit_fixture_generation(&fixture, initial_request)
        .expect("write initial two-owner source-index snapshot");

    let mut pruned_request = large_refresh_request(&project_root, 1);
    pruned_request.file_count = 1;
    pruned_request.import.file_hashes = vec![survivor_file_hash.clone()];
    pruned_request.import.source_blobs = Default::default();
    pruned_request.import.owners.clear();
    pruned_request.import.selectors.clear();
    let pruned_snapshot = initial_workspace_snapshot
        .with_overlay_delta(
            std::iter::empty::<(String, String)>(),
            ["src/generated/owner_1.rs"],
        )
        .evidence(
            agent_semantic_content_identity::SourceSnapshotKind::Filesystem,
            "d".repeat(64),
        );
    pruned_request.import.generation_id =
        agent_semantic_client_db::client_db_source_index_generation_id_for_snapshot(
            &pruned_snapshot,
        );
    pruned_request.source_snapshot = pruned_snapshot.clone();
    let pruned = commit_fixture_generation(&fixture, pruned_request)
        .expect("publish pruned source-index snapshot");
    assert!(!pruned.reused_generation);
    assert_eq!(pruned.owner_count, 1);
    assert_eq!(pruned.selector_count, 1);
    assert_eq!(pruned.changed_owner_count, 0);
    assert_eq!(pruned.removed_owner_count, 1);
    assert_eq!(pruned.posting_write_count, 0);

    let rust_language_id = LanguageId::from("rust");
    let lookup_deadline = Instant::now() + Duration::from_secs(15);
    let lookup = loop {
        let lookup = ClientDbEngine::lookup_source_index_read_model_from_client_dir(
            &client_dir,
            &pruned.source_snapshot,
            "source_index_large_owner_1",
            Some(&rust_language_id),
            8,
        )
        .await
        .expect("lookup removed source-index owner");
        if lookup.state != ClientDbSourceIndexLookupState::Busy {
            break lookup;
        }
        assert!(
            Instant::now() < lookup_deadline,
            "removed-owner lookup remained busy past the test deadline"
        );
        tokio::time::sleep(Duration::from_millis(10)).await;
    };
    assert_eq!(lookup.state, ClientDbSourceIndexLookupState::Miss);
    assert!(lookup.candidates.is_empty(), "lookup={lookup:?}");

    let _ = std::fs::remove_dir_all(root);
}

#[tokio::test(flavor = "current_thread")]
#[ignore = "explicit source-index memory-bound gate with a 300-owner fixture"]
async fn source_index_lookup_bounds_query_bytes_terms_and_candidate_limit() {
    let _test_guard = source_index_refresh_test_guard();
    let root = temp_project_root("source-index-lookup-memory-bounds");
    let client_dir = root.join("client");
    let fixture =
        agent_semantic_client_db::fixture::SourceIndexFixture::for_client_dir(&client_dir);
    let project_root = root.join("project");
    std::fs::create_dir_all(&client_dir).expect("create client dir");
    std::fs::create_dir_all(project_root.join("src")).expect("create project src dir");

    let mut request = large_refresh_request(&project_root, 300);
    for owner in &mut request.import.owners {
        owner.query_keys.push("shared_lookup_token".into());
    }
    for selector in &mut request.import.selectors {
        selector.query_keys.push("shared_lookup_token".into());
    }
    commit_fixture_generation(&fixture, request).expect("write source-index memory-bound fixture");

    let rust_language_id = LanguageId::from("rust");
    let lookup_deadline = Instant::now() + Duration::from_secs(15);
    let bounded = loop {
        let lookup = ClientDbEngine::lookup_source_index_read_model_from_client_dir(
            &client_dir,
            &crate::snapshot_fixture::source_snapshot_evidence(),
            "shared_lookup_token",
            Some(&rust_language_id),
            u32::MAX,
        )
        .await
        .expect("lookup with oversized candidate request");
        if lookup.state != ClientDbSourceIndexLookupState::Busy {
            break lookup;
        }
        assert!(
            Instant::now() < lookup_deadline,
            "source-index candidate lookup remained busy past the test deadline"
        );
        tokio::time::sleep(Duration::from_millis(10)).await;
    };
    assert_eq!(bounded.state, ClientDbSourceIndexLookupState::Hit);
    assert_eq!(bounded.candidates.len(), 256);

    let term_bounded_query = (0..32)
        .map(|index| format!("absent_term_{index}"))
        .chain(std::iter::once("shared_lookup_token".to_string()))
        .collect::<Vec<_>>()
        .join(" ");
    let term_bounded = ClientDbEngine::lookup_source_index_read_model_from_client_dir(
        &client_dir,
        &crate::snapshot_fixture::source_snapshot_evidence(),
        &term_bounded_query,
        Some(&rust_language_id),
        8,
    )
    .await
    .expect("lookup with excess source-index terms");
    assert_eq!(term_bounded.state, ClientDbSourceIndexLookupState::Miss);
    assert!(term_bounded.candidates.is_empty());

    let oversized_query = "x".repeat(16 * 1024 + 1);
    let error = ClientDbEngine::lookup_source_index_read_model_from_client_dir(
        &client_dir,
        &crate::snapshot_fixture::source_snapshot_evidence(),
        &oversized_query,
        Some(&rust_language_id),
        8,
    )
    .await
    .expect_err("oversized source-index query should fail before token projection");
    assert!(
        error.contains("source-index query exceeds byte budget"),
        "error={error}"
    );

    let _ = std::fs::remove_dir_all(root);
}

#[tokio::test(flavor = "current_thread")]
async fn source_index_duplicate_owner_fails_before_cold_write() {
    let _test_guard = source_index_refresh_test_guard();
    let root = temp_project_root("source-index-refresh-rollback");
    let client_dir = root.join("client");
    let fixture =
        agent_semantic_client_db::fixture::SourceIndexFixture::for_client_dir(&client_dir);
    let project_root = root.join("project");
    std::fs::create_dir_all(&client_dir).expect("create client dir");
    std::fs::create_dir_all(project_root.join("src")).expect("create project src dir");

    let mut invalid_request = refresh_request(&project_root);
    invalid_request
        .import
        .owners
        .push(invalid_request.import.owners[0].clone());
    let failed_snapshot = invalid_request.source_snapshot.clone();
    let error = commit_fixture_generation(&fixture, invalid_request)
        .expect_err("duplicate owner must fail before the cold write");
    assert!(
        error.contains("duplicate owner path=src/source_index_perf.rs"),
        "unexpected duplicate-owner admission error: {error}"
    );

    let language_id = LanguageId::from("rust");
    let lookup = ClientDbEngine::lookup_source_index_read_model_from_client_dir(
        &client_dir,
        &failed_snapshot,
        "source_index_perf_fixture",
        Some(&language_id),
        8,
    )
    .await
    .expect("lookup after failed cold write");
    assert_ne!(
        lookup.state.as_str(),
        "hit",
        "failed materialization admission must not expose partial source-index rows"
    );

    let retry = commit_fixture_generation(&fixture, refresh_request(&project_root))
        .expect("retry after rollback");
    assert!(
        !retry.reused_generation,
        "retry must write a complete generation after failed admission"
    );

    let _ = std::fs::remove_dir_all(root);
}
