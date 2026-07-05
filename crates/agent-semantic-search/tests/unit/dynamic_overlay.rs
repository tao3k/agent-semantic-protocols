use agent_semantic_search::{
    DynamicOverlayLane, QUERY_OVERLAY_ROUTE_SOURCE, SEARCH_OVERLAY_ROUTE_SOURCE,
};

#[test]
fn dynamic_overlay_lanes_expose_search_and_query_route_sources() {
    assert_eq!(
        DynamicOverlayLane::Search.route_source(),
        SEARCH_OVERLAY_ROUTE_SOURCE
    );
    assert_eq!(
        DynamicOverlayLane::Query.route_source(),
        QUERY_OVERLAY_ROUTE_SOURCE
    );
}
