use std::path::PathBuf;
use std::time::Instant;

use agent_semantic_search::file_locator::FileLocatorIndex;
use agent_semantic_search::search_planner::{
    SearchPlannerRequest, SearchPlannerRoute, plan_search_route,
};
use agent_semantic_search::{
    SourceIndexClientCacheLookupRequest, SourceIndexClientCachePlannerLookupRequest,
    lookup_source_index_in_client_cache_dir_with_planner,
};

#[test]
fn planner_routes_filename_query_to_file_locator_before_source_index() {
    let locator = FileLocatorIndex::build(vec![
        PathBuf::from("src/search/planner.rs"),
        PathBuf::from("src/lib.rs"),
    ]);

    let decision = plan_search_route(SearchPlannerRequest {
        query: "planner.rs",
        limit: 4,
        file_locator: Some(&locator),
    });

    assert_eq!(decision.route, SearchPlannerRoute::FileLocator);
    assert_eq!(
        decision.file_candidates[0].workspace_relative_path,
        "src/search/planner.rs"
    );
}

#[test]
fn planner_falls_through_to_source_index_when_file_locator_misses() {
    let locator = FileLocatorIndex::build(vec![PathBuf::from("src/lib.rs")]);

    let decision = plan_search_route(SearchPlannerRequest {
        query: "CacheStatus",
        limit: 4,
        file_locator: Some(&locator),
    });

    assert_eq!(decision.route, SearchPlannerRoute::SourceIndex);
    assert!(decision.file_candidates.is_empty());
}

#[test]
fn planner_file_locator_hot_path_stays_under_two_milliseconds() {
    let mut paths = (0..20_000)
        .map(|index| PathBuf::from(format!("packages/pkg-{index}/src/module_{index}.rs")))
        .collect::<Vec<_>>();
    paths.push(PathBuf::from(
        "crates/agent-semantic-search/src/search_planner.rs",
    ));
    let locator = FileLocatorIndex::build(paths);

    let started = Instant::now();
    let decision = plan_search_route(SearchPlannerRequest {
        query: "search_planner.rs",
        limit: 8,
        file_locator: Some(&locator),
    });
    let elapsed = started.elapsed();

    assert_eq!(decision.route, SearchPlannerRoute::FileLocator);
    assert!(
        elapsed.as_micros() < 2_000,
        "planner file locator hot path took {elapsed:?}, expected < 2ms"
    );
}

#[test]
fn source_index_adapter_uses_file_locator_on_cache_miss() {
    let project_root = tempfile::tempdir().expect("project tempdir");
    let cache_root = tempfile::tempdir().expect("cache tempdir");
    let locator = FileLocatorIndex::build(vec![PathBuf::from("src/search_planner.rs")]);

    let lookup = lookup_source_index_in_client_cache_dir_with_planner(
        SourceIndexClientCachePlannerLookupRequest {
            source_index: SourceIndexClientCacheLookupRequest {
                cache_root: cache_root.path(),
                indexed_project_root: project_root.path(),
                language_id: None,
                query: "search_planner.rs",
                limit: 8,
            },
            file_locator: Some(&locator),
        },
    )
    .expect("lookup with file locator planner");

    assert_eq!(
        lookup.state,
        agent_semantic_client_db::ClientDbSourceIndexLookupState::Hit
    );
    assert_eq!(lookup.candidates[0].path, "src/search_planner.rs");
}
