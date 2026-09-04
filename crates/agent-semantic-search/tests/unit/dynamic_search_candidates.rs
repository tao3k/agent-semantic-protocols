use std::fs;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;

use crate::DynamicSearchRootCandidateRequest;
use crate::RgCoverageBudget;
use crate::RgCoverageOwner;
use crate::RgCoverageRequest;
use crate::collect_dynamic_lexical_overlay_candidates_from_roots;
use crate::collect_rg_coverage_candidates;

#[test]
fn committed_lexical_overlay_projects_path_and_content_without_executable_ranges() {
    let root = std::env::temp_dir().join(format!(
        "asp-dynamic-candidates-{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after unix epoch")
            .as_nanos()
    ));
    let src = root.join("src");
    fs::create_dir_all(&src).expect("create source directory");
    let owner = src.join("dynamic_overlay_owner.rs");
    fs::write(
        &owner,
        "pub fn dynamic_owner_item_index() { let overlay = true; }\n",
    )
    .expect("write owner fixture");

    let terms = vec!["dynamic".to_string()];
    let owner_bytes = fs::read(&owner).expect("read owner fixture");
    let snapshot = agent_semantic_content_identity::WorkspaceSnapshot::from_file_bytes([(
        "src/dynamic_overlay_owner.rs",
        owner_bytes,
    )]);
    let fixture = crate::source_snapshot_fixture::canonical_test_snapshot();
    let owners = vec![std::path::PathBuf::from("src/dynamic_overlay_owner.rs")];
    let collection =
        collect_dynamic_lexical_overlay_candidates_from_roots(DynamicSearchRootCandidateRequest {
            project_root: &root,
            locator_root: &root,
            terms: &terms,
            owners: &owners,
            ignore_dirs: &[],
            include_hidden_dirs: &[],
            base_snapshot: &snapshot,
            provider_digest: fixture.provider_digest.as_str(),
            file_matches: &|path| path.extension().and_then(|value| value.to_str()) == Some("rs"),
            limit: 8,
        })
        .expect("collect committed lexical candidates");
    let candidates = collection.candidates;

    assert!(
        candidates
            .iter()
            .any(|candidate| candidate.source == "search-overlay"
                && candidate.confidence == "path-lexical-overlay"
                && candidate.path == "src/dynamic_overlay_owner.rs")
    );
    assert!(
        candidates
            .iter()
            .any(|candidate| candidate.source == "search-overlay"
                && candidate.confidence == "lexical-overlay"
                && candidate.text.contains("dynamic_owner_item_index"))
    );
    assert!(candidates.iter().all(|candidate| {
        candidate.line == 1 && candidate.end_line == 1 && !candidate.path.contains(":1:1")
    }));

    fs::remove_dir_all(root).expect("remove fixture");
}

#[test]
fn explicit_owner_candidates_apply_language_filter_without_workspace_walk() {
    let root = std::env::temp_dir().join(format!(
        "asp-dynamic-root-candidates-{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after unix epoch")
            .as_nanos()
    ));
    let src = root.join("src");
    let ignored = root.join("target");
    fs::create_dir_all(&src).expect("create source directory");
    fs::create_dir_all(&ignored).expect("create ignored directory");
    fs::write(
        src.join("dynamic_overlay_owner.rs"),
        "pub fn dynamic_owner_item_index() {}\n",
    )
    .expect("write rust fixture");
    fs::write(src.join("dynamic_overlay_owner.txt"), "dynamic text\n")
        .expect("write non-rust fixture");
    fs::write(ignored.join("dynamic_ignored.rs"), "pub fn ignored() {}\n")
        .expect("write ignored fixture");

    let terms = vec!["dynamic".to_string()];
    let ignore_dirs = vec!["target".to_string()];
    let include_hidden_dirs = Vec::new();
    let owners = vec![std::path::PathBuf::from("src/dynamic_overlay_owner.rs")];
    let fixture = crate::source_snapshot_fixture::canonical_test_snapshot();
    let owner_bytes = fs::read(src.join("dynamic_overlay_owner.rs")).expect("read owner fixture");
    let snapshot = agent_semantic_content_identity::WorkspaceSnapshot::from_file_bytes([(
        "src/dynamic_overlay_owner.rs",
        owner_bytes,
    )]);
    let collection =
        collect_dynamic_lexical_overlay_candidates_from_roots(DynamicSearchRootCandidateRequest {
            project_root: &root,
            locator_root: &root,
            terms: &terms,
            owners: &owners,
            ignore_dirs: &ignore_dirs,
            include_hidden_dirs: &include_hidden_dirs,
            base_snapshot: &snapshot,
            provider_digest: fixture.provider_digest.as_str(),
            file_matches: &|path| {
                path.extension().and_then(|extension| extension.to_str()) == Some("rs")
            },
            limit: 8,
        })
        .expect("collect root candidates");
    let candidates = collection.candidates;

    assert!(
        candidates
            .iter()
            .any(|candidate| candidate.path == "src/dynamic_overlay_owner.rs")
    );
    assert!(
        candidates
            .iter()
            .all(|candidate| !candidate.path.ends_with(".txt")
                && !candidate.path.contains("target/"))
    );

    fs::remove_dir_all(root).expect("remove fixture");
}

#[test]
fn dynamic_overlay_rejects_directory_scans_in_query_path() {
    let root = tempfile::tempdir().expect("temporary workspace");
    let owners = vec![std::path::PathBuf::from("src")];
    fs::create_dir_all(root.path().join("src")).expect("source directory");
    let terms = vec!["owner".to_owned()];
    let fixture = crate::source_snapshot_fixture::canonical_test_snapshot();

    let failure =
        collect_dynamic_lexical_overlay_candidates_from_roots(DynamicSearchRootCandidateRequest {
            project_root: root.path(),
            locator_root: root.path(),
            terms: &terms,
            owners: &owners,
            ignore_dirs: &[],
            include_hidden_dirs: &[],
            base_snapshot: &fixture.workspace,
            provider_digest: fixture.provider_digest.as_str(),
            file_matches: &|path| path.extension().and_then(|value| value.to_str()) == Some("rs"),
            limit: 8,
        })
        .expect_err("directory scan must be removed from the query path");

    assert!(failure.contains("directory scan is removed"));
}

#[test]
fn dynamic_overlay_rejects_owner_bytes_that_drift_from_merkle_snapshot() {
    let root = tempfile::tempdir().expect("temporary workspace");
    let owner = root.path().join("src/owner.rs");
    fs::create_dir_all(owner.parent().expect("owner parent")).expect("source directory");
    fs::write(&owner, "pub fn committed_owner() {}\n").expect("committed owner");
    let snapshot = agent_semantic_content_identity::WorkspaceSnapshot::from_file_bytes([(
        "src/owner.rs",
        b"pub fn committed_owner() {}\n",
    )]);
    fs::write(&owner, "pub fn stale_owner() {}\n").expect("drift owner");
    let fixture = crate::source_snapshot_fixture::canonical_test_snapshot();
    let owners = vec![std::path::PathBuf::from("src/owner.rs")];
    let terms = vec!["owner".to_owned()];

    let failure =
        collect_dynamic_lexical_overlay_candidates_from_roots(DynamicSearchRootCandidateRequest {
            project_root: root.path(),
            locator_root: root.path(),
            terms: &terms,
            owners: &owners,
            ignore_dirs: &[],
            include_hidden_dirs: &[],
            base_snapshot: &snapshot,
            provider_digest: fixture.provider_digest.as_str(),
            file_matches: &|_| true,
            limit: 8,
        })
        .expect_err("stale owner bytes must fail closed");

    assert!(failure.contains("owner content drift"));
}

#[test]
fn rg_coverage_candidates_are_explicit_bounded_and_generation_bound() {
    let root = std::env::temp_dir().join(format!(
        "asp-ingest-candidates-{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after unix epoch")
            .as_nanos()
    ));
    let src = root.join("src");
    fs::create_dir_all(&src).expect("create source directory");
    fs::write(src.join("owner.rs"), "pub fn owner_symbol() {}\n").expect("write owner fixture");

    let owner_source = b"\n\n\n\n\n\n\n\n\n\n\npub fn owner_symbol() {}\n";
    let stdin = b"src/owner.rs:12:7:pub fn owner_symbol() {}\0";
    let fixture = crate::source_snapshot_fixture::canonical_test_snapshot();
    let snapshot = agent_semantic_content_identity::WorkspaceSnapshot::from_file_bytes([(
        "src/owner.rs",
        owner_source,
    )]);
    let owner_digest = snapshot
        .file_digest("src/owner.rs")
        .expect("committed owner digest")
        .to_owned();
    let evidence = snapshot.evidence(
        agent_semantic_content_identity::SourceSnapshotKind::Filesystem,
        fixture.provider_digest.clone(),
    );
    let owners = [RgCoverageOwner {
        owner_path: "src/owner.rs",
        content_digest: &owner_digest,
        source: owner_source,
    }];
    let result = collect_rg_coverage_candidates(RgCoverageRequest {
        locator_root: &root,
        output: stdin,
        generation_digest: "generation-1",
        source_snapshot: &evidence,
        workspace_snapshot: &snapshot,
        owners: &owners,
        budget: RgCoverageBudget::new(4096, 16, 8).expect("bounded rg budget"),
    })
    .expect("bounded rg coverage result");
    let candidates = result.candidates;

    assert_eq!(candidates.len(), 1);
    assert_eq!(candidates[0].path, "src/owner.rs");
    assert_eq!(candidates[0].line, 12);
    assert_eq!(candidates[0].end_line, 12);
    assert_eq!(candidates[0].symbol, "pub");
    assert_eq!(candidates[0].source, "rg-query");
    assert_eq!(result.receipt.state, "ready");
    assert_eq!(result.receipt.generation_digest, "generation-1");
    assert_eq!(result.receipt.source_root_digest, evidence.root_digest);
    assert_eq!(result.receipt.provider_digest, evidence.provider_digest);
    assert_eq!(
        result.receipt.index_artifact_digest,
        agent_semantic_search_projection::source_index_artifact_digest(&evidence)
    );
    assert_eq!(result.receipt.committed_owner_count, 1);
    assert!(
        result
            .receipt
            .coverage_input_digest
            .starts_with("blake3-256:")
    );

    fs::remove_dir_all(root).expect("remove fixture");
}

#[test]
fn rg_coverage_budget_failure_is_typed_and_returns_no_partial_candidates() {
    let fixture = crate::source_snapshot_fixture::canonical_test_snapshot();
    let source = b"a\nb\n";
    let snapshot =
        agent_semantic_content_identity::WorkspaceSnapshot::from_file_bytes([("src/a.rs", source)]);
    let digest = snapshot
        .file_digest("src/a.rs")
        .expect("owner digest")
        .to_owned();
    let evidence = snapshot.evidence(
        agent_semantic_content_identity::SourceSnapshotKind::Filesystem,
        fixture.provider_digest.clone(),
    );
    let owners = [RgCoverageOwner {
        owner_path: "src/a.rs",
        content_digest: &digest,
        source,
    }];
    let failure = collect_rg_coverage_candidates(RgCoverageRequest {
        locator_root: std::path::Path::new("/workspace"),
        output: b"src/a.rs:1:1:a\nsrc/b.rs:2:1:b\n",
        generation_digest: "generation-2",
        source_snapshot: &evidence,
        workspace_snapshot: &snapshot,
        owners: &owners,
        budget: RgCoverageBudget::new(4096, 1, 8).expect("bounded rg budget"),
    })
    .expect_err("record overflow must fail closed");

    assert_eq!(failure.state, "failed");
    assert_eq!(failure.reason_kind, Some("record-budget-exceeded"));
    assert_eq!(failure.generation_digest, "generation-2");
    assert_eq!(failure.candidate_count, 0);
}

#[test]
fn rg_coverage_rejects_stale_lines_without_partial_candidates() {
    let fixture = crate::source_snapshot_fixture::canonical_test_snapshot();
    let source = b"pub fn current_owner() {}\n";
    let snapshot = agent_semantic_content_identity::WorkspaceSnapshot::from_file_bytes([(
        "src/owner.rs",
        source,
    )]);
    let digest = snapshot
        .file_digest("src/owner.rs")
        .expect("owner digest")
        .to_owned();
    let evidence = snapshot.evidence(
        agent_semantic_content_identity::SourceSnapshotKind::Filesystem,
        fixture.provider_digest.clone(),
    );
    let owners = [RgCoverageOwner {
        owner_path: "src/owner.rs",
        content_digest: &digest,
        source,
    }];

    let failure = collect_rg_coverage_candidates(RgCoverageRequest {
        locator_root: std::path::Path::new("/workspace"),
        output: b"src/owner.rs:1:1:pub fn stale_owner() {}\n",
        generation_digest: "generation-3",
        source_snapshot: &evidence,
        workspace_snapshot: &snapshot,
        owners: &owners,
        budget: RgCoverageBudget::new(4096, 16, 8).expect("bounded rg budget"),
    })
    .expect_err("stale rg line must fail closed");

    assert_eq!(failure.reason_kind, Some("record-content-mismatch"));
    assert_eq!(failure.candidate_count, 0);
}

#[test]
fn rg_coverage_candidate_overflow_fails_without_truncated_success() {
    let fixture = crate::source_snapshot_fixture::canonical_test_snapshot();
    let source = b"first\nsecond\n";
    let snapshot = agent_semantic_content_identity::WorkspaceSnapshot::from_file_bytes([(
        "src/owner.rs",
        source,
    )]);
    let digest = snapshot
        .file_digest("src/owner.rs")
        .expect("owner digest")
        .to_owned();
    let evidence = snapshot.evidence(
        agent_semantic_content_identity::SourceSnapshotKind::Filesystem,
        fixture.provider_digest.clone(),
    );
    let owners = [RgCoverageOwner {
        owner_path: "src/owner.rs",
        content_digest: &digest,
        source,
    }];

    let failure = collect_rg_coverage_candidates(RgCoverageRequest {
        locator_root: std::path::Path::new("/workspace"),
        output: b"src/owner.rs:1:1:first\nsrc/owner.rs:2:1:second\n",
        generation_digest: "generation-4",
        source_snapshot: &evidence,
        workspace_snapshot: &snapshot,
        owners: &owners,
        budget: RgCoverageBudget::new(4096, 16, 1).expect("bounded rg budget"),
    })
    .expect_err("candidate overflow must not return truncated success");

    assert_eq!(failure.reason_kind, Some("candidate-budget-exceeded"));
    assert_eq!(failure.candidate_count, 0);
}
