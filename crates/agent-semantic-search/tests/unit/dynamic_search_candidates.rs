use std::fs;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::{
    DynamicSearchCandidateRequest, DynamicSearchRootCandidateRequest,
    collect_dynamic_lexical_overlay_candidates,
    collect_dynamic_lexical_overlay_candidates_from_roots, collect_ingest_search_candidates,
};

#[test]
fn lexical_overlay_candidates_project_path_and_content_hits_without_executable_ranges() {
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
    let search_roots = vec![vec![owner]];
    let fixture = crate::source_snapshot_fixture::canonical_test_snapshot();
    let collection = collect_dynamic_lexical_overlay_candidates(DynamicSearchCandidateRequest {
        locator_root: &root,
        terms: &terms,
        search_roots: &search_roots,
        base_snapshot: &fixture.workspace,
        provider_digest: fixture.provider_digest.as_str(),
        limit: 8,
    });
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
fn root_candidates_walk_workspace_with_language_filter_and_ignore_dirs() {
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
    let fixture = crate::source_snapshot_fixture::canonical_test_snapshot();
    let collection =
        collect_dynamic_lexical_overlay_candidates_from_roots(DynamicSearchRootCandidateRequest {
            project_root: &root,
            locator_root: &root,
            terms: &terms,
            owners: &[],
            ignore_dirs: &ignore_dirs,
            include_hidden_dirs: &include_hidden_dirs,
            base_snapshot: &fixture.workspace,
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
fn ingest_candidates_parse_line_and_path_records_in_client_core() {
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

    let stdin = b"src/owner.rs:12:7:pub fn owner_symbol() {}\0src/owner.rs\nmissing.rs\n";
    let candidates = collect_ingest_search_candidates(&root, &root, stdin, 8);

    assert_eq!(candidates.len(), 2);
    assert_eq!(candidates[0].path, "src/owner.rs");
    assert_eq!(candidates[0].line, 12);
    assert_eq!(candidates[0].end_line, 12);
    assert_eq!(candidates[0].symbol, "pub");
    assert_eq!(candidates[0].source, "ingest");
    assert_eq!(candidates[1].path, "src/owner.rs");
    assert_eq!(candidates[1].line, 1);
    assert_eq!(candidates[1].end_line, 1);
    assert_eq!(candidates[1].symbol, "src");

    fs::remove_dir_all(root).expect("remove fixture");
}
