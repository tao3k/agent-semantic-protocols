use agent_semantic_client_db::TursoClientDbGraphEntity;

use agent_semantic_search::{
    GraphOwnerItemEvidence, GraphOwnerItemRenderRequest, GraphOwnerItemRoute,
    GraphOwnerItemRouteRequest, GraphSemanticKind, rank_graph_owner_items,
    render_graph_owner_item_frontier,
};

fn selector_node(
    id: &str,
    symbol: &str,
    semantic_kind: &str,
    selector: &str,
) -> TursoClientDbGraphEntity {
    TursoClientDbGraphEntity {
        id: id.to_string(),
        kind: "selector".to_string(),
        semantic_kind: Some(semantic_kind.to_string()),
        label: symbol.to_string(),
        selector: Some(selector.to_string()),
        path: Some("src/runtime.ss".to_string()),
        language_id: Some("gerbil-scheme".to_string()),
        provider_id: Some("gerbil-scheme-language-project-harness".to_string()),
        query_keys: vec![semantic_kind.to_string(), symbol.to_string()],
    }
}

#[test]
fn graph_owner_item_frontier_has_exact_selectors_without_line_ranges() {
    let route = GraphOwnerItemRoute::Hit(vec![GraphOwnerItemEvidence {
        node_id: "item:run".to_string(),
        owner_path: "src/runtime.ss".to_string(),
        symbol: "run".to_string(),
        semantic_kind: GraphSemanticKind::new("function").expect("semantic kind"),
        selector: "gerbil-scheme://src/runtime.ss#item/function/run".to_string(),
    }]);
    let output = render_graph_owner_item_frontier(GraphOwnerItemRenderRequest {
        language_id: "gerbil-scheme",
        owner_path: "src/runtime.ss",
        query: "run",
        route: &route,
    });
    assert!(output.contains("structuralSelector=gerbil-scheme://src/runtime.ss#item/function/run"));
    assert!(output.contains("--from-hook item-skeleton"));
    assert!(!output.contains("displayLineRange"));
}

#[test]
fn graph_owner_items_rank_parser_identity_without_selector_parsing() {
    let nodes = vec![
        selector_node(
            "item:render",
            "render_dynamic_owner_items_frontier",
            "function",
            "gerbil-scheme://src/runtime.ss#item/function/render_dynamic_owner_items_frontier",
        ),
        selector_node(
            "item:owner",
            "DynamicOwnerItem",
            "struct",
            "gerbil-scheme://src/runtime.ss#item/struct/DynamicOwnerItem",
        ),
    ];
    let route = rank_graph_owner_items(GraphOwnerItemRouteRequest {
        owner_path: "src/runtime.ss",
        query_terms: &["render_dynamic_owner_items_frontier".to_string()],
        nodes: &nodes,
    });
    assert_eq!(
        route,
        GraphOwnerItemRoute::Hit(vec![GraphOwnerItemEvidence {
            node_id: "item:render".to_string(),
            owner_path: "src/runtime.ss".to_string(),
            symbol: "render_dynamic_owner_items_frontier".to_string(),
            semantic_kind: GraphSemanticKind::new("function").expect("semantic kind"),
            selector:
                "gerbil-scheme://src/runtime.ss#item/function/render_dynamic_owner_items_frontier"
                    .to_string(),
        }])
    );
}

#[test]
fn graph_owner_items_reject_nodes_without_parser_item_identity() {
    let mut node = selector_node(
        "item:missing-kind",
        "run",
        "function",
        "gerbil-scheme://src/runtime.ss#item/function/run",
    );
    node.semantic_kind = None;
    let route = rank_graph_owner_items(GraphOwnerItemRouteRequest {
        owner_path: "src/runtime.ss",
        query_terms: &["run".to_string()],
        nodes: &[node],
    });
    assert_eq!(route, GraphOwnerItemRoute::Empty);
}
