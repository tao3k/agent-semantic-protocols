use crate::{GraphProjectionCandidate, ranked_graph_owner_paths_for_submodule_paths};

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
