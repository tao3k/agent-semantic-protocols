use super::{
    SelectorSeedCursorBinding, SelectorSeededSearchCursorRequest, SelectorSeededSearchPipeRequest,
    render_selector_seed_topology, selector_seed_topology, selector_seeded_search_cursor,
};

use agent_semantic_loop::search_graph_cursor::{
    GraphRouteBudget, GraphRouteBudgetRequest, SearchGraphCursorJumpRequest,
};

fn selector_request<'a>(selector: &'a str) -> SelectorSeededSearchPipeRequest<'a> {
    SelectorSeededSearchPipeRequest {
        language_id: "rust",
        selector,
        query: "render selector topology",
        workspace: ".",
    }
}

fn cursor_binding(state_digest: &str) -> SelectorSeedCursorBinding {
    SelectorSeedCursorBinding {
        state_digest: state_digest.to_string(),
        graph_generation: 7,
        workspace_generation: "workspace-generation-42".to_string(),
        remaining_budget: 4,
        route_budget: GraphRouteBudget::new(GraphRouteBudgetRequest {
            max_evidence_tokens: 4096,
            max_raw_tokens: 8192,
            max_uncached_tokens: 2048,
            max_rounds: 8,
            max_graph_hops: 4,
            max_transitions: 8,
        }),
    }
}

#[test]
fn canonical_selector_builds_stable_graph_and_typed_actions() {
    let selector = "rust://src/search.rs#item/function/render";
    let first = selector_seed_topology(selector_request(selector));
    let second = selector_seed_topology(selector_request(selector));

    assert_eq!(first.nodes, second.nodes);
    assert_eq!(first.edges, second.edges);
    assert_eq!(first.action_frontier, second.action_frontier);
    for relation in ["serves_owner", "owns_item", "targets_item", "materializes"] {
        assert!(
            first.edges.iter().any(|edge| edge["relation"] == relation),
            "missing relation {relation}: {:?}",
            first.edges
        );
    }
    assert_eq!(first.action_frontier[0].kind, "query-code");
    assert_eq!(first.action_frontier[0].capability_id, "query");
    assert_eq!(first.action_frontier[0].target, selector);
    assert_eq!(first.action_frontier[1].kind, "owner-items");
    assert!(first.nodes.iter().any(|node| {
        node["kind"] == "item"
            && node["structuralSelector"] == selector
            && node["requiresExact"] == true
    }));
}

#[test]
fn invalid_or_cross_language_selector_has_no_false_owner_or_action() {
    for selector in [
        "typescript://src/index.ts#item/function/render",
        "rust://#item/function/render",
    ] {
        let topology = selector_seed_topology(selector_request(selector));
        assert!(topology.actions.is_empty(), "{selector}: {topology:?}");
        assert!(
            topology.action_frontier.is_empty(),
            "{selector}: {topology:?}"
        );
        assert!(
            topology
                .nodes
                .iter()
                .all(|node| node["kind"] != "owner" && node["kind"] != "item"),
            "{selector}: {:?}",
            topology.nodes
        );
        assert!(topology.edges.iter().all(|edge| {
            edge["relation"] != "serves_owner"
                && edge["relation"] != "owns_item"
                && edge["relation"] != "targets_item"
                && edge["relation"] != "materializes"
        }));
    }
}

#[test]
fn compact_render_projects_the_typed_frontier_without_reclassification() {
    let topology = selector_seed_topology(selector_request(
        "rust://src/search.rs#item/function/render",
    ));
    let output = render_selector_seed_topology(&topology);

    assert!(output.contains("actionFrontier=A1.query-code,A2.owner-items"));
    assert!(output.contains("recommendedNext=A1.query-code"));
    assert!(output.contains("nextCommand=asp rust query --selector"));
    assert!(output.contains("--projection source"));
    assert!(!output.contains(" --code"));
}

#[test]
fn selector_projection_bootstraps_cursor_over_the_same_action_nodes() {
    let cursor = selector_seeded_search_cursor(SelectorSeededSearchCursorRequest {
        search: selector_request("rust://src/search.rs#item/function/render"),
        binding: cursor_binding("selector-state-7"),
    })
    .expect("bootstrap selector cursor");

    assert_eq!(cursor.state().frontier().len(), 2);
    assert!(
        cursor
            .materialized_node_ids()
            .contains(cursor.state().current_node())
    );
    assert!(
        cursor
            .state()
            .visited()
            .iter()
            .any(|node| node == cursor.state().current_node())
    );
    let target = cursor.state().frontier()[0].clone();
    let jumped = cursor
        .jump(SearchGraphCursorJumpRequest {
            target_node_id: &target,
            next_state_digest: "selector-state-7-action",
        })
        .expect("jump to typed action node");
    assert_eq!(jumped.state().current_node(), target);
    assert_eq!(jumped.state().route_measure().tool_rounds(), 0);
    assert_eq!(jumped.state().remaining_budget(), 4);
}

#[test]
fn invalid_selector_cursor_has_no_executable_action_frontier() {
    let cursor = selector_seeded_search_cursor(SelectorSeededSearchCursorRequest {
        search: selector_request("typescript://src/index.ts#item/function/render"),
        binding: cursor_binding("selector-invalid-state"),
    })
    .expect("bootstrap unresolved selector cursor");

    assert!(cursor.state().frontier().is_empty());
    assert_eq!(cursor.state().route_measure().executed_graph_hops(), 0);
}
