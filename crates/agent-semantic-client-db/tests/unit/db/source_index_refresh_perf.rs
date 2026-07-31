use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use agent_semantic_client_core::{CacheGenerationId, ClientCacheFileHash, LanguageId, ProviderId};
use agent_semantic_client_db::{
    ClientDbEngine, ClientDbSourceIndexImport, ClientDbSourceIndexLookupState,
    ClientDbSourceIndexOwner, ClientDbSourceIndexRefreshRequest, ClientDbSourceIndexSelector,
};

const SOURCE_INDEX_WARM_REUSE_GATE: Duration = Duration::from_millis(750);
const SOURCE_INDEX_HASH_REUSE_GATE: Duration = Duration::from_millis(25);
#[cfg(not(debug_assertions))]
const SOURCE_INDEX_1193_OWNER_COLD_WRITE_GATE: Duration = Duration::from_secs(1);
#[cfg(not(debug_assertions))]
const SOURCE_INDEX_POSTING_FRONTIER_COLD_WRITE_GATE: Duration = Duration::from_secs(1);
#[cfg(not(debug_assertions))]
const SOURCE_INDEX_1193_OWNER_ONE_PERCENT_REFRESH_GATE: Duration = Duration::from_millis(500);
#[cfg(not(debug_assertions))]
const SOURCE_INDEX_1193_OWNER_HIGH_FANOUT_COLD_LOOKUP_GATE: Duration = Duration::from_millis(400);

static SOURCE_INDEX_REFRESH_TEST_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

async fn source_index_refresh_test_guard() -> tokio::sync::MutexGuard<'static, ()> {
    SOURCE_INDEX_REFRESH_TEST_LOCK.lock().await
}

#[cfg(not(debug_assertions))]
#[tokio::test(flavor = "current_thread")]
async fn source_index_1193_owner_cold_write_stays_inside_v1_gate() {
    let _test_guard = source_index_refresh_test_guard().await;
    let root = temp_project_root("source-index-1193-owner-cold-write");
    let client_dir = root.join("client");
    let fixture =
        agent_semantic_client_db::fixture::SourceIndexFixture::for_client_dir(&client_dir);
    let project_root = root.join("project");
    std::fs::create_dir_all(&client_dir).expect("create client dir");
    std::fs::create_dir_all(project_root.join("src")).expect("create project src dir");

    let started_at = Instant::now();
    let refresh = commit_fixture_generation(&fixture, large_refresh_request(&project_root, 1_193))
        .expect("write 1193-owner source-index snapshot");
    let elapsed = started_at.elapsed();

    assert!(!refresh.reused_generation, "cold import must publish rows");
    assert_eq!(refresh.owner_count, 1_193);
    assert_eq!(refresh.selector_count, 1_193);
    assert_eq!(refresh.changed_owner_count, 1_193);
    assert_eq!(refresh.removed_owner_count, 0);
    assert_eq!(
        refresh.posting_write_count, 6_308,
        "cold-write receipt must match the deterministic 16-term owner frontier"
    );
    assert!(
        elapsed < SOURCE_INDEX_1193_OWNER_COLD_WRITE_GATE,
        "1193-owner source-index cold write exceeded V1 DB gate: elapsed={elapsed:?} gate={SOURCE_INDEX_1193_OWNER_COLD_WRITE_GATE:?}"
    );

    let _ = std::fs::remove_dir_all(root);
}

#[cfg(not(debug_assertions))]
#[tokio::test(flavor = "current_thread")]
async fn source_index_1278_owner_posting_frontier_cold_write_stays_inside_v1_gate() {
    let _test_guard = source_index_refresh_test_guard().await;
    let root = temp_project_root("source-index-1278-owner-posting-frontier-cold-write");
    let client_dir = root.join("client");
    let fixture =
        agent_semantic_client_db::fixture::SourceIndexFixture::for_client_dir(&client_dir);
    let project_root = root.join("project");
    std::fs::create_dir_all(&client_dir).expect("create client dir");
    std::fs::create_dir_all(project_root.join("src")).expect("create project src dir");

    let request = large_high_term_refresh_request(&project_root, 1_278, 32);
    let started_at = Instant::now();
    let refresh = commit_fixture_generation(&fixture, request)
        .expect("write bounded-posting source-index snapshot");
    let elapsed = started_at.elapsed();

    assert_eq!(refresh.owner_count, 1_278);
    assert_eq!(refresh.selector_count, 1_278);
    assert!(
        (12_000..=20_448).contains(&refresh.posting_write_count),
        "posting frontier must remain large enough to exercise bulk writes while retaining at most 16 ranked terms per owner: refresh={refresh:?}"
    );
    assert!(
        elapsed < SOURCE_INDEX_POSTING_FRONTIER_COLD_WRITE_GATE,
        "posting-frontier source-index cold write exceeded V1 DB gate: elapsed={elapsed:?} gate={SOURCE_INDEX_POSTING_FRONTIER_COLD_WRITE_GATE:?}"
    );

    let _ = std::fs::remove_dir_all(root);
}

#[cfg(not(debug_assertions))]
#[tokio::test(flavor = "current_thread")]
async fn source_index_1193_owner_one_percent_refresh_stays_inside_v1_gate() {
    let _test_guard = source_index_refresh_test_guard().await;
    let root = temp_project_root("source-index-1193-owner-one-percent-refresh");
    let client_dir = root.join("client");
    let fixture =
        agent_semantic_client_db::fixture::SourceIndexFixture::for_client_dir(&client_dir);
    let project_root = root.join("project");
    std::fs::create_dir_all(&client_dir).expect("create client dir");
    std::fs::create_dir_all(project_root.join("src")).expect("create project src dir");

    commit_fixture_generation(&fixture, large_refresh_request(&project_root, 1_193))
        .expect("write initial 1193-owner source-index snapshot");

    let mut changed_request = large_refresh_request(&project_root, 1_193);
    for (index, file_hash) in changed_request
        .import
        .file_hashes
        .iter_mut()
        .take(12)
        .enumerate()
    {
        file_hash.sha256 = format!("{:064x}", u64::MAX - index as u64);
    }

    let started_at = Instant::now();
    let refresh = commit_fixture_generation(&fixture, changed_request)
        .expect("refresh one-percent changed source-index snapshot");
    let elapsed = started_at.elapsed();

    assert!(
        !refresh.reused_generation,
        "changed import must publish rows"
    );
    assert_eq!(refresh.owner_count, 1_193);
    assert_eq!(refresh.selector_count, 1_193);
    assert_eq!(
        refresh.changed_owner_count, 12,
        "the one-percent scenario must serialize only its 12 changed owners"
    );
    assert_eq!(refresh.removed_owner_count, 0);
    assert_eq!(
        refresh.posting_write_count, 204,
        "12 changed owners with 17 canonical terms each must rebuild only 204 postings"
    );
    assert!(
        elapsed < SOURCE_INDEX_1193_OWNER_ONE_PERCENT_REFRESH_GATE,
        "1193-owner one-percent refresh exceeded V1 DB gate: elapsed={elapsed:?} gate={SOURCE_INDEX_1193_OWNER_ONE_PERCENT_REFRESH_GATE:?}"
    );

    let _ = std::fs::remove_dir_all(root);
}

#[cfg(not(debug_assertions))]
#[tokio::test(flavor = "current_thread")]
async fn source_index_1193_owner_high_fanout_lookup_stays_inside_v1_gate() {
    let _test_guard = source_index_refresh_test_guard();
    let root = temp_project_root("source-index-1193-owner-high-fanout-lookup");
    let client_dir = root.join("client");
    let fixture =
        agent_semantic_client_db::fixture::SourceIndexFixture::for_client_dir(&client_dir);
    let project_root = root.join("project");
    std::fs::create_dir_all(&client_dir).expect("create client dir");
    std::fs::create_dir_all(project_root.join("src")).expect("create project src dir");

    commit_fixture_generation(&fixture, large_refresh_request(&project_root, 1_193))
        .expect("write high-fanout source-index snapshot");

    let cold_started_at = Instant::now();
    let cold_lookup = ClientDbEngine::lookup_source_index_read_model_from_client_dir(
        &client_dir,
        &crate::snapshot_fixture::source_snapshot_evidence(),
        "source",
        Some(&LanguageId::from("rust")),
        128,
    )
    .await
    .expect("read high-fanout relational postings");
    let cold_elapsed = cold_started_at.elapsed();

    assert!(
        cold_lookup.candidates.len() >= 128,
        "high-fanout lookup must return the bounded owner window: lookup={cold_lookup:?}"
    );
    assert!(
        cold_elapsed < SOURCE_INDEX_1193_OWNER_HIGH_FANOUT_COLD_LOOKUP_GATE,
        "1193-owner high-fanout cold lookup exceeded V1 DB gate: elapsed={cold_elapsed:?} gate={SOURCE_INDEX_1193_OWNER_HIGH_FANOUT_COLD_LOOKUP_GATE:?}"
    );

    let _ = std::fs::remove_dir_all(root);
}

#[tokio::test(flavor = "current_thread")]
async fn source_index_refresh_reuse_stays_on_structured_turso_path() {
    let _test_guard = source_index_refresh_test_guard();
    let root = temp_project_root("source-index-refresh-reuse-perf");
    let client_dir = root.join("client");
    let fixture =
        agent_semantic_client_db::fixture::SourceIndexFixture::for_client_dir(&client_dir);
    let project_root = root.join("project");
    std::fs::create_dir_all(&client_dir).expect("create client dir");
    std::fs::create_dir_all(project_root.join("src")).expect("create project src dir");

    let request = refresh_request(&project_root);
    let first =
        commit_fixture_generation(&fixture, request.clone()).expect("cold source-index refresh");
    assert!(!first.reused_generation, "first refresh should write rows");

    let started_at = Instant::now();
    let second = commit_fixture_generation(&fixture, request).expect("warm source-index refresh");
    let elapsed = started_at.elapsed();

    assert!(second.reused_generation, "second refresh should reuse rows");
    assert_eq!(second.owner_count, 1);
    assert_eq!(second.selector_count, 1);
    assert!(
        elapsed < SOURCE_INDEX_WARM_REUSE_GATE,
        "warm source-index refresh should stay on structured Turso path; elapsed={elapsed:?} gate={SOURCE_INDEX_WARM_REUSE_GATE:?}"
    );

    let _ = std::fs::remove_dir_all(root);
}

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

    let initial_snapshot = crate::snapshot_fixture::source_snapshot_evidence_for_files(1, 2);
    let mut initial_request = large_refresh_request(&project_root, 2);
    initial_request.import.generation_id =
        agent_semantic_client_db::client_db_source_index_generation_id_for_snapshot(
            &initial_snapshot,
        );
    initial_request.source_snapshot = initial_snapshot;
    commit_fixture_generation(&fixture, initial_request)
        .expect("write initial two-owner source-index snapshot");

    let mut pruned_request = large_refresh_request(&project_root, 2);
    let pruned_snapshot = crate::snapshot_fixture::source_snapshot_evidence_for(2);
    pruned_request.import.generation_id =
        agent_semantic_client_db::client_db_source_index_generation_id_for_snapshot(
            &pruned_snapshot,
        );
    pruned_request.source_snapshot = pruned_snapshot.clone();
    pruned_request.file_count = 1;
    pruned_request.import.file_hashes.pop();
    pruned_request.import.owners.pop();
    pruned_request.import.selectors.pop();
    let pruned = commit_fixture_generation(&fixture, pruned_request)
        .expect("publish pruned source-index snapshot");
    assert!(!pruned.reused_generation);
    assert_eq!(pruned.owner_count, 1);
    assert_eq!(pruned.selector_count, 1);
    assert_eq!(
        pruned.changed_owner_count, 1,
        "an immutable generation must write every surviving owner row"
    );
    assert_eq!(pruned.removed_owner_count, 1);
    assert!(
        pruned.posting_write_count > 0,
        "an immutable generation must write postings for every surviving owner"
    );

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
async fn source_index_incomplete_materialization_fails_before_cold_write() {
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
    let error = commit_fixture_generation(&fixture, invalid_request)
        .expect_err("incomplete materialization must fail before the cold write");
    assert!(
        error.contains("workspace canonical materialization is incomplete"),
        "unexpected materialization admission error: {error}"
    );

    let language_id = LanguageId::from("rust");
    let lookup = ClientDbEngine::lookup_source_index_read_model_from_client_dir(
        &client_dir,
        &crate::snapshot_fixture::source_snapshot_evidence(),
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
        path: source_path.clone(),
        language_id: LanguageId::from("rust"),
        provider_id: ProviderId::from("rs-harness"),
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
        path: source_path.clone(),
        language_id: LanguageId::from("rust"),
        provider_id: ProviderId::from("rs-harness"),
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
    changed_request.import.file_hashes[0].sha256 = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb".to_string();
    let changed_snapshot = crate::snapshot_fixture::source_snapshot_evidence_for(2);
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
fn source_index_refresh_rejects_selector_bytes_without_matching_file_identity() {
    let _test_guard = source_index_refresh_test_guard();
    let root = temp_project_root("source-index-selector-only-refresh");
    let client_dir = root.join("client");
    let fixture =
        agent_semantic_client_db::fixture::SourceIndexFixture::for_client_dir(&client_dir);
    let project_root = root.join("project");
    std::fs::create_dir_all(&client_dir).expect("create client dir");
    std::fs::create_dir_all(project_root.join("src")).expect("create project src dir");

    commit_fixture_generation(&fixture, refresh_request(&project_root))
        .expect("write initial source-index facts");

    let mut changed_request = refresh_request(&project_root);
    changed_request.import.selectors[0].source =
        "pub fn source_index_perf_fixture() { changed(); }".into();
    let error = commit_fixture_generation(&fixture, changed_request)
        .expect_err("selector bytes without matching file identity must fail closed");
    assert!(
        error.contains("immutable workspace generation materialization drift"),
        "unexpected selector materialization error: {error}"
    );

    let _ = std::fs::remove_dir_all(root);
}

fn refresh_request(project_root: &Path) -> ClientDbSourceIndexRefreshRequest {
    let structural_selector =
        "rust://src/source_index_perf.rs#item/function/source_index_perf_fixture";
    ClientDbSourceIndexRefreshRequest {
        file_count: 1,
        source_snapshot: crate::snapshot_fixture::source_snapshot_evidence(),
        import: ClientDbSourceIndexImport {
            generation_id: CacheGenerationId::from("snapshot-bound-source-index-perf"),
            project_root: project_root.to_path_buf(),
            schema_id: "agent-semantic-client-db.source-index".to_string().into(),
            schema_version: "1".into(),
            file_hashes: vec![ClientCacheFileHash {
                path: "src/source_index_perf.rs".to_string(),
                sha256: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_string(),
                byte_len: 42,
                mtime_ms: 1,
            }],
            owners: vec![ClientDbSourceIndexOwner {
                owner_path: "src/source_index_perf.rs".into(),
                language_id: Some(LanguageId::from("rust")),
                provider_id: Some(ProviderId::from("rs-harness")),
                source_kind: "source".to_string().into(),
                line_count: Some(8),
                query_keys: vec!["source_index_perf_fixture".to_string().into()],
            }],
            selectors: vec![ClientDbSourceIndexSelector {
                owner_path: "src/source_index_perf.rs".into(),
                selector_id: structural_selector.into(),
                symbol: Some("source_index_perf_fixture".into()),
                kind: Some("function".into()),
                source: "pub fn source_index_perf_fixture() {}".to_string().into(),
                query_keys: vec!["source_index_perf_fixture".to_string().into()],
                materialization_proof: crate::materialization_fixture::materialization_proof(
                    crate::materialization_fixture::MaterializationFixtureInput {
                        language_id: "rust",
                        provider_id: "rs-harness",
                        owner_path: "src/source_index_perf.rs",
                        structural_selector,
                        item_kind: "function",
                        item_name: "source_index_perf_fixture",
                        source: b"pub fn source_index_perf_fixture() {}",
                        source_byte_start: 0,
                        source_byte_end: b"pub fn source_index_perf_fixture() {}".len() as u64,
                    },
                ),
            }],
        },
    }
}

fn commit_fixture_generation(
    fixture: &agent_semantic_client_db::fixture::SourceIndexFixture,
    request: ClientDbSourceIndexRefreshRequest,
) -> Result<agent_semantic_client_db::ClientDbSourceIndexRefreshReport, String> {
    let source_blobs = agent_semantic_client_db::ClientDbSourceIndexSourceBlobs::from_normalized(
        request.import.selectors.iter().map(|selector| {
            (
                selector.materialization_proof.owner_path.clone().into(),
                selector.materialization_proof.projection.clone(),
            )
        }),
    );
    fixture.commit_source_index_generation(request, &source_blobs)
}

fn large_refresh_request(
    project_root: &Path,
    owner_count: u32,
) -> ClientDbSourceIndexRefreshRequest {
    let mut file_hashes = Vec::with_capacity(owner_count as usize);
    let mut owners = Vec::with_capacity(owner_count as usize);
    let mut selectors = Vec::with_capacity(owner_count as usize);
    for index in 0..owner_count {
        let owner_path = format!("src/generated/owner_{index}.rs");
        let symbol = format!("source_index_large_owner_{index}");
        let selector_id = format!("rust://{owner_path}#item/function/{symbol}");
        let source = format!("pub fn {symbol}() {{}}");
        file_hashes.push(ClientCacheFileHash {
            path: owner_path.clone(),
            sha256: format!("{index:064x}"),
            byte_len: 64,
            mtime_ms: u64::from(index) + 1,
        });
        owners.push(ClientDbSourceIndexOwner {
            owner_path: owner_path.clone().into(),
            language_id: Some(LanguageId::from("rust")),
            provider_id: Some(ProviderId::from("rs-harness")),
            source_kind: "source".to_string().into(),
            line_count: Some(1),
            query_keys: vec![symbol.clone().into()],
        });
        selectors.push(ClientDbSourceIndexSelector {
            owner_path: owner_path.into(),
            selector_id: selector_id.clone().into(),
            symbol: Some(symbol.clone().into()),
            kind: Some("function".into()),
            source: source.clone().into(),
            query_keys: vec![symbol.into()],
            materialization_proof: crate::materialization_fixture::materialization_proof(
                crate::materialization_fixture::MaterializationFixtureInput {
                    language_id: "rust",
                    provider_id: "rs-harness",
                    owner_path: &format!("src/generated/owner_{index}.rs"),
                    structural_selector: &selector_id,
                    item_kind: "function",
                    item_name: &format!("source_index_large_owner_{index}"),
                    source: source.as_bytes(),
                    source_byte_start: 0,
                    source_byte_end: source.len() as u64,
                },
            ),
        });
    }
    ClientDbSourceIndexRefreshRequest {
        file_count: owner_count,
        source_snapshot: crate::snapshot_fixture::source_snapshot_evidence(),
        import: ClientDbSourceIndexImport {
            generation_id: CacheGenerationId::from("snapshot-bound-source-index-large"),
            project_root: project_root.to_path_buf(),
            schema_id: "agent-semantic-client-db.source-index".to_string().into(),
            schema_version: "1".into(),
            file_hashes,
            owners,
            selectors,
        },
    }
}

#[cfg(not(debug_assertions))]
fn large_high_term_refresh_request(
    project_root: &Path,
    owner_count: u32,
    unique_terms_per_owner: u32,
) -> ClientDbSourceIndexRefreshRequest {
    let mut request = large_refresh_request(project_root, owner_count);
    for (owner_index, owner) in request.import.owners.iter_mut().enumerate() {
        owner.query_keys.extend(
            (0..unique_terms_per_owner)
                .map(|term_index| format!("z{owner_index:04}q{term_index:02}unique").into()),
        );
    }
    request
}

fn run_git(project_root: &Path, args: impl IntoIterator<Item = &'static str>) {
    let output = Command::new("git")
        .arg("-C")
        .arg(project_root)
        .args(args)
        .output()
        .expect("run git for source-index fixture");
    assert!(
        output.status.success(),
        "git source-index fixture command failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

fn temp_project_root(label: &str) -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time")
        .as_nanos();
    std::env::temp_dir().join(format!("asp-{label}-{nonce}"))
}
