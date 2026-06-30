use std::collections::HashSet;
use std::fs;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::{
    QueryCandidateAppend, QueryWrapperCandidate, QueryWrapperCandidateSurface,
    QueryWrapperQualityCandidate, QueryWrapperScanConfig, QueryWrapperSearchClause,
    QueryWrapperSourceIndexLookup, QueryWrapperSourceIndexRequest, analyze_query_wrapper_quality,
    append_query_candidates, augment_package_path_candidates,
    collect_query_wrapper_source_index_candidates, query_candidate_priority,
    query_wrapper_axis_terms, query_wrapper_candidate_matches_term, query_wrapper_package_key,
    query_wrapper_terms,
};

#[test]
fn query_wrapper_terms_split_and_dedupe_raw_queries() {
    assert_eq!(
        query_wrapper_terms("CacheStatus cache_status,cacheStatus|owner"),
        vec![
            "cachestatus".to_string(),
            "cache_status".to_string(),
            "owner".to_string(),
        ]
    );
}

#[test]
fn query_wrapper_axis_terms_expand_identifier_components() {
    let terms = query_wrapper_axis_terms("CacheStatus cache_status HTTPServer");

    assert!(terms.contains(&"cachestatus".to_string()));
    assert!(terms.contains(&"cache".to_string()));
    assert!(terms.contains(&"status".to_string()));
    assert!(terms.contains(&"cache_status".to_string()));
    assert!(terms.contains(&"httpserver".to_string()));
    assert!(terms.contains(&"http".to_string()));
    assert!(terms.contains(&"server".to_string()));
}

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
fn query_wrapper_quality_flags_broad_noisy_flat_recall() {
    let clauses = vec![QueryWrapperSearchClause {
        id: 1,
        terms: vec![
            "search".to_string(),
            "provider".to_string(),
            "owner".to_string(),
            "policy".to_string(),
        ],
        axis_terms: Vec::new(),
    }];
    let candidates = vec![
        QueryWrapperQualityCandidate {
            path: "analyzers/julia/search_owner.jl".to_string(),
            symbol: "search_owner".to_string(),
            text: "provider owner policy".to_string(),
        },
        QueryWrapperQualityCandidate {
            path: "packages/client/runtime/search.rs".to_string(),
            symbol: "search_runtime".to_string(),
            text: "provider search".to_string(),
        },
        QueryWrapperQualityCandidate {
            path: "packages/protocol/command/search.rs".to_string(),
            symbol: "owner_policy".to_string(),
            text: "owner policy".to_string(),
        },
        QueryWrapperQualityCandidate {
            path: "docs/search/provider.org".to_string(),
            symbol: String::new(),
            text: "search provider".to_string(),
        },
        QueryWrapperQualityCandidate {
            path: "languages/python/search_provider.py".to_string(),
            symbol: "provider".to_string(),
            text: "provider".to_string(),
        },
    ];

    let quality = analyze_query_wrapper_quality(&[], &clauses, &clauses[0].terms, &candidates);

    assert_eq!(quality.query_pack_quality, "low");
    assert_eq!(quality.scope_quality, "low");
    assert_eq!(quality.package_cohesion, "low");
    assert!(!quality.allow_query_selector);
    assert!(quality.risks.contains(&"single-flat-or-recall".to_string()));
    assert!(quality.risks.contains(&"broad-scope".to_string()));
    assert!(quality.risks.contains(&"low-package-cohesion".to_string()));
    assert!(quality.risks.contains(&"generic-terms".to_string()));
    assert!(quality.risks.contains(&"noisy-candidates".to_string()));
    assert_eq!(quality.noise, vec!["analyzers/julia".to_string()]);
    assert_eq!(
        query_wrapper_package_key("packages/client/runtime/search.rs"),
        "packages/client/runtime"
    );
    assert!(query_wrapper_candidate_matches_term(
        &candidates[0],
        "owner"
    ));
}

#[test]
fn query_wrapper_quality_allows_focused_multi_clause_selector() {
    let clauses = vec![
        QueryWrapperSearchClause {
            id: 1,
            terms: vec!["state".to_string(), "manifest".to_string()],
            axis_terms: Vec::new(),
        },
        QueryWrapperSearchClause {
            id: 2,
            terms: vec!["workspace".to_string(), "identity".to_string()],
            axis_terms: Vec::new(),
        },
    ];
    let candidates = vec![
        QueryWrapperQualityCandidate {
            path: "crates/agent-semantic-client-core/src/state_core.rs".to_string(),
            symbol: "write_state_manifest".to_string(),
            text: "state manifest workspace identity".to_string(),
        },
        QueryWrapperQualityCandidate {
            path: "crates/agent-semantic-client-core/src/state_identity.rs".to_string(),
            symbol: "workspace_identity".to_string(),
            text: "workspace identity state".to_string(),
        },
    ];
    let terms = clauses
        .iter()
        .flat_map(|clause| clause.terms.iter().cloned())
        .collect::<Vec<_>>();

    let quality = analyze_query_wrapper_quality(
        &[std::path::PathBuf::from(
            "crates/agent-semantic-client-core",
        )],
        &clauses,
        &terms,
        &candidates,
    );

    assert_eq!(quality.query_pack_quality, "high");
    assert_eq!(quality.scope_quality, "high");
    assert_eq!(quality.package_cohesion, "high");
    assert!(quality.allow_query_selector);
    assert!(quality.risks.is_empty());
    assert_eq!(quality.clause_coverages.len(), 2);
    assert!(
        quality
            .clause_coverages
            .iter()
            .all(|coverage| coverage.missing.is_empty())
    );
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
