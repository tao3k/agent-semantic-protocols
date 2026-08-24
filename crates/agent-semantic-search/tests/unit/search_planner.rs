use std::path::PathBuf;
use std::time::Instant;

use agent_semantic_search::file_locator::FileLocatorIndex;
use agent_semantic_search::search_planner::{
    SearchPlannerRequest, SearchPlannerRoute, SemanticStorageRouteRequest, plan_search_route,
    plan_semantic_storage_route,
};
use agent_semantic_search_projection::{
    SemanticMutationClass, SemanticSearchAlgorithmEvidence, SemanticSearchQueryRoute,
    SemanticSearchStorageProfile, SemanticSharingScope,
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
fn planner_enters_lexical_search_frame_when_file_locator_misses() {
    let locator = FileLocatorIndex::build(vec![PathBuf::from("src/lib.rs")]);

    let decision = plan_search_route(SearchPlannerRequest {
        query: "CacheStatus",
        limit: 4,
        file_locator: Some(&locator),
    });

    assert_eq!(decision.route, SearchPlannerRoute::LexicalSearchFrame);
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

fn storage_profile(
    mutation_class: SemanticMutationClass,
    sharing_scope: SemanticSharingScope,
    tree_depth: u32,
    traversal_radius: u32,
) -> SemanticSearchStorageProfile {
    SemanticSearchStorageProfile {
        mutation_class,
        sharing_scope,
        identity_digest: "blake3-256:route-fixture".to_owned(),
        algorithm_evidence: SemanticSearchAlgorithmEvidence {
            tree_depth,
            traversal_radius,
            graph_node_count: 100,
            graph_edge_count: 120,
            merkle_delta_owner_count: 1,
            dependency_fan_out: 4,
            cross_workspace_reuse_count: 0,
            component_count: Some(1),
            cut_vertex_count: Some(0),
            provider_graph_evidence: Vec::new(),
        },
    }
}

#[test]
fn static_versioned_dependency_depth_does_not_force_memory() {
    let decision = plan_semantic_storage_route(SemanticStorageRouteRequest {
        profile: storage_profile(
            SemanticMutationClass::Immutable,
            SemanticSharingScope::PackageVersion,
            12,
            5,
        ),
        has_exact_owner_path: false,
        has_structural_selector: false,
    })
    .expect("static dependency route");

    assert_eq!(
        decision.query_route,
        SemanticSearchQueryRoute::StaticDatabaseIndex
    );
}

#[test]
fn dynamic_workspace_depth_or_graph_radius_routes_to_resident_memory() {
    for (tree_depth, traversal_radius) in [(2, 1), (1, 2)] {
        let decision = plan_semantic_storage_route(SemanticStorageRouteRequest {
            profile: storage_profile(
                SemanticMutationClass::GenerationBound,
                SemanticSharingScope::WorkspaceGeneration,
                tree_depth,
                traversal_radius,
            ),
            has_exact_owner_path: false,
            has_structural_selector: false,
        })
        .expect("resident graph route");
        assert_eq!(
            decision.query_route,
            SemanticSearchQueryRoute::ResidentMemory
        );
    }
}

#[test]
fn shallow_generation_navigation_routes_to_database() {
    let decision = plan_semantic_storage_route(SemanticStorageRouteRequest {
        profile: storage_profile(
            SemanticMutationClass::GenerationBound,
            SemanticSharingScope::WorkspaceGeneration,
            1,
            1,
        ),
        has_exact_owner_path: false,
        has_structural_selector: false,
    })
    .expect("shallow DB route");

    assert_eq!(
        decision.query_route,
        SemanticSearchQueryRoute::ShallowDatabaseIndex
    );
}

#[test]
fn exact_path_and_selector_bypass_db_and_graph_planning() {
    let decision = plan_semantic_storage_route(SemanticStorageRouteRequest {
        profile: storage_profile(
            SemanticMutationClass::OwnerDynamic,
            SemanticSharingScope::OwnerContent,
            8,
            8,
        ),
        has_exact_owner_path: true,
        has_structural_selector: true,
    })
    .expect("exact mmap route");

    assert_eq!(decision.query_route, SemanticSearchQueryRoute::ExactMmap);
}

#[test]
fn dense_graph_or_broad_merkle_delta_prevents_shallow_db_route() {
    let mut dense = storage_profile(
        SemanticMutationClass::GenerationBound,
        SemanticSharingScope::WorkspaceGeneration,
        1,
        1,
    );
    dense.algorithm_evidence.graph_edge_count = 900;
    let mut broad_delta = dense.clone();
    broad_delta.algorithm_evidence.graph_edge_count = 120;
    broad_delta.algorithm_evidence.merkle_delta_owner_count = 6;

    for profile in [dense, broad_delta] {
        let decision = plan_semantic_storage_route(SemanticStorageRouteRequest {
            profile,
            has_exact_owner_path: false,
            has_structural_selector: false,
        })
        .expect("resident route for expensive dynamic graph");
        assert_eq!(
            decision.query_route,
            SemanticSearchQueryRoute::ResidentMemory
        );
    }
}
