use std::collections::HashSet;
use std::fs;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::{
    QueryCandidateAppend, QueryWrapperCandidate, QueryWrapperCandidateSurface,
    QueryWrapperScanConfig, QueryWrapperSourceIndexLookup, QueryWrapperSourceIndexRequest,
    append_query_candidates, augment_package_path_candidates,
    collect_query_wrapper_source_index_candidates, query_candidate_priority,
};

#[test]
fn query_wrapper_scan_respects_ignore_dirs_and_language_files() {
    let root = temp_root("asp-query-wrapper-scan");
    let src = root.join("src");
    let ignored = root.join("target");
    fs::create_dir_all(&src).expect("create source directory");
    fs::create_dir_all(&ignored).expect("create ignored directory");
    fs::write(
        src.join("query_wrapper_owner.rs"),
        "pub fn query_wrapper_owner() {}\n",
    )
    .expect("write rust fixture");
    fs::write(src.join("query_wrapper_notes.txt"), "query wrapper text\n")
        .expect("write unsupported fixture");
    fs::write(
        ignored.join("query_wrapper_ignored.rs"),
        "pub fn query_wrapper_ignored() {}\n",
    )
    .expect("write ignored fixture");

    let terms = vec!["query_wrapper".to_string()];
    let ignore_dirs = vec!["target".to_string()];
    let include_hidden_dirs = Vec::new();
    let mut seen = HashSet::new();
    let mut candidates = Vec::new();
    append_query_candidates(QueryCandidateAppend {
        surface: QueryWrapperCandidateSurface::Fd,
        locator_root: &root,
        path: &root,
        terms: &terms,
        axis_terms: &terms,
        config: QueryWrapperScanConfig {
            ignore_dirs: &ignore_dirs,
            include_hidden_dirs: &include_hidden_dirs,
        },
        accept_all_files: false,
        seen: &mut seen,
        candidates: &mut candidates,
    })
    .expect("append query wrapper candidates");

    assert!(
        candidates
            .iter()
            .any(|candidate| candidate.path == "src/query_wrapper_owner.rs")
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
fn package_path_augmentation_adds_only_missing_package_axis() {
    let root = temp_root("asp-query-wrapper-package");
    let package_dir = root.join("src").join("query_wrapper_pkg");
    fs::create_dir_all(&package_dir).expect("create package directory");
    fs::write(package_dir.join("mod.rs"), "pub mod query_wrapper_pkg {}\n")
        .expect("write package fixture");

    let terms = vec!["query_wrapper_pkg".to_string()];
    let ignore_dirs = Vec::new();
    let include_hidden_dirs = Vec::new();
    let mut candidates = vec![QueryWrapperCandidate {
        path: "src/other.rs".to_string(),
        line: 1,
        end_line: 1,
        symbol: "other".to_string(),
        text: "other".to_string(),
        source: "fd-query".to_string(),
        confidence: "path".to_string(),
    }];
    let added = augment_package_path_candidates(
        &root,
        &[root.clone()],
        &terms,
        QueryWrapperScanConfig {
            ignore_dirs: &ignore_dirs,
            include_hidden_dirs: &include_hidden_dirs,
        },
        &mut candidates,
    )
    .expect("augment package path candidates");

    assert_eq!(added, 1);
    assert!(candidates.iter().any(|candidate| {
        candidate.path == "src/query_wrapper_pkg/mod.rs"
            && candidate.source == "package-path-query"
            && candidate.confidence == "package-path"
    }));

    fs::remove_dir_all(root).expect("remove fixture");
}

#[test]
fn query_candidate_priority_prefers_axis_coverage_and_runtime_source() {
    let terms = vec!["cache".to_string(), "status".to_string()];
    let axis_terms = terms.clone();
    let src_priority = query_candidate_priority("src/cache/status.rs", &terms, &axis_terms);
    let test_priority = query_candidate_priority("tests/cache.rs", &terms, &axis_terms);
    let partial_priority = query_candidate_priority("src/cache.rs", &terms, &axis_terms);

    assert!(src_priority < test_priority);
    assert!(src_priority < partial_priority);
}

#[test]
fn source_index_query_collection_returns_none_for_missing_db_without_creating_cache() {
    let root = temp_root("asp-query-wrapper-source-index");
    fs::create_dir_all(root.join("src")).expect("create source directory");
    fs::write(
        root.join("src").join("lib.rs"),
        "pub fn query_wrapper_source_index() {}\n",
    )
    .expect("write source fixture");

    let terms = vec!["query_wrapper_source_index".to_string()];
    let lookup = QueryWrapperSourceIndexLookup {
        db_path: root.join("client.sqlite3"),
        state: "missing-db".to_string(),
        candidates: Vec::new(),
    };
    let collection =
        collect_query_wrapper_source_index_candidates(QueryWrapperSourceIndexRequest {
            surface: QueryWrapperCandidateSurface::Rg,
            project_root: &root,
            roots: std::slice::from_ref(&root),
            terms: &terms,
            axis_terms: &terms,
            lookup: &lookup,
        })
        .expect("collect source-index candidates");

    assert!(collection.is_none());
    assert!(!root.join(".cache").join("agent-semantic-protocol").exists());

    fs::remove_dir_all(root).expect("remove fixture");
}

fn temp_root(prefix: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!(
        "{}-{}",
        prefix,
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after unix epoch")
            .as_nanos()
    ))
}
