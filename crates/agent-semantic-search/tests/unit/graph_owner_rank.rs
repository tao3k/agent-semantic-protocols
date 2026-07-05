use crate::graph_candidate_projection::GraphProjectionCandidate;
use crate::graph_owner_rank::ranked_graph_owner_paths_for_submodule_paths;

fn candidate(
    path: &str,
    symbol: &str,
    text: &str,
    source: &str,
    confidence: &str,
) -> crate::graph_owner_rank::GraphOwnerRankCandidate {
    crate::graph_owner_rank::GraphOwnerRankCandidate {
        path: path.to_owned(),
        symbol: symbol.to_owned(),
        text: text.to_owned(),
        source: source.to_owned(),
        confidence: confidence.to_owned(),
    }
}

#[test]
fn graph_owner_rank_prefers_package_query_axis_coverage() {
    let candidates = vec![
        GraphProjectionCandidate::new(
            "packages/runtime/db/src/lib.rs",
            1,
            1,
            "unrelated",
            "db connection",
            "source-index",
            "high",
        ),
        GraphProjectionCandidate::new(
            "packages/runtime/search/src/router.rs",
            1,
            1,
            "SearchRouter",
            "dynamic overlay graph ranking",
            "source-index",
            "high",
        ),
        GraphProjectionCandidate::new(
            "packages/runtime/search/src/overlay.rs",
            1,
            1,
            "OverlaySearch",
            "dynamic overlay",
            "finder-path",
            "path",
        ),
    ];
    let ranked = ranked_graph_owner_paths_for_submodule_paths(
        &candidates,
        &["dynamicOverlay".to_string(), "SearchRouter".to_string()],
        &[],
    );

    assert_eq!(ranked[0], "packages/runtime/search/src/router.rs");
    assert_eq!(ranked[1], "packages/runtime/search/src/overlay.rs");
}

#[test]
fn graph_owner_rank_prefers_topology_local_submodule_matches() {
    let candidates = vec![
        GraphProjectionCandidate::new(
            "src/lib.rs",
            1,
            1,
            "SearchRouter",
            "dynamic overlay graph ranking",
            "source-index",
            "high",
        ),
        GraphProjectionCandidate::new(
            "languages/rust/src/lib.rs",
            1,
            1,
            "SearchRouter",
            "dynamic overlay graph ranking",
            "source-index",
            "high",
        ),
    ];
    let ranked = ranked_graph_owner_paths_for_submodule_paths(
        &candidates,
        &["dynamicOverlay".to_string()],
        &["languages/rust".to_string()],
    );

    assert_eq!(ranked[0], "languages/rust/src/lib.rs");
}

#[test]
fn graph_owner_rank_report_projects_score_breakdown_for_python_consumers() {
    let candidates = vec![
        candidate(
            "src/lib.rs",
            "SearchRouter",
            "dynamic overlay graph ranking",
            "source-index",
            "high",
        ),
        candidate(
            "languages/rust/src/lib.rs",
            "SearchRouter",
            "dynamic overlay graph ranking",
            "source-index",
            "high",
        ),
    ];

    let report = crate::graph_owner_rank::rank_graph_owner_report(
        crate::graph_owner_rank::GraphOwnerRankRequest {
            candidates,
            query_terms: vec!["dynamicOverlay".to_string()],
            submodule_paths: vec!["languages/rust".to_string()],
        },
    );

    assert_eq!(
        report.query_axes,
        vec!["dynamic", "overlay", "dynamicoverlay"]
    );
    let top = report
        .ranked_owners
        .first()
        .expect("graph owner rank report should include ranked owners");
    assert_eq!(top.path, "languages/rust/src/lib.rs");
    assert_eq!(top.package_root, "languages/rust");
    assert_eq!(
        top.topology_submodule_path.as_deref(),
        Some("languages/rust")
    );
    assert_eq!(top.matched_query_axes, vec!["dynamic", "overlay"]);
    assert_eq!(top.symbols, vec!["SearchRouter"]);
    assert_eq!(top.score.query_axis_count, 2);
    assert_eq!(top.score.topology_local_hits, 1);
    assert_eq!(top.score.parser_finder_local_hits, 1);
    assert!(top.score.total > 0);
}

#[test]
fn graph_owner_rank_hot_path_stays_under_twenty_milliseconds() {
    let candidate_count = if cfg!(debug_assertions) { 1_024 } else { 8_192 };
    let mut candidates = Vec::with_capacity(candidate_count);
    for index in 0..candidate_count {
        let path = if index % 8 == 0 {
            format!("languages/rust/src/search/overlay_{index}.rs")
        } else if index % 3 == 0 {
            format!("packages/runtime/search/src/router_{index}.rs")
        } else {
            format!("packages/runtime/db/src/connection_{index}.rs")
        };
        let symbol = if index % 8 == 0 {
            format!("DynamicOverlaySearch{index}")
        } else if index % 3 == 0 {
            format!("SearchRouter{index}")
        } else {
            format!("DbConnection{index}")
        };
        let text = if index % 8 == 0 {
            "dynamic overlay graph ranking local topology"
        } else if index % 3 == 0 {
            "dynamic overlay graph ranking"
        } else {
            "db connection pool"
        };

        candidates.push(candidate(&path, &symbol, text, "source-index", "high"));
    }

    let query_terms = ["dynamicOverlay".to_string()];
    let submodule_paths = ["languages/rust".to_string()];

    let started_at = std::time::Instant::now();
    let report = crate::graph_owner_rank::rank_graph_owner_report(
        crate::graph_owner_rank::GraphOwnerRankRequest {
            candidates,
            query_terms: query_terms.to_vec(),
            submodule_paths: submodule_paths.to_vec(),
        },
    );
    let elapsed = started_at.elapsed();

    assert_eq!(report.ranked_owners.len(), candidate_count);
    assert!(
        report.ranked_owners[0]
            .path
            .starts_with("languages/rust/src/search/overlay_")
    );
    assert!(
        elapsed < std::time::Duration::from_millis(20),
        "graph owner rank should stay under 20ms for {candidate_count} candidates, took {elapsed:?}"
    );
}
