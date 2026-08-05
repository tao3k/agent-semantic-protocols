use super::{GraphSelectorSeedProjectionRequest, graph_selector_seed_projection};

#[test]
fn canonical_selector_projects_stable_owner_item_and_intent_relations() {
    let request = GraphSelectorSeedProjectionRequest {
        language_id: "rust",
        selector: "rust://src/search.rs#item/function/render",
        query: "render selector topology",
        workspace: ".",
    };
    let first = graph_selector_seed_projection(request);
    let second = graph_selector_seed_projection(request);

    assert_eq!(first.nodes, second.nodes);
    assert_eq!(first.edges, second.edges);
    assert_eq!(first.owner.as_deref(), Some("src/search.rs"));
    assert_eq!(first.symbol.as_deref(), Some("render"));
    for relation in ["serves_owner", "owns_item", "targets_item"] {
        assert!(
            first.edges.iter().any(|edge| edge["relation"] == relation),
            "missing {relation}: {first:?}"
        );
    }
}

#[test]
fn invalid_or_cross_language_selector_never_projects_owner_or_item() {
    for selector in [
        "typescript://src/index.ts#item/function/render",
        "rust://#item/function/render",
    ] {
        let projection = graph_selector_seed_projection(GraphSelectorSeedProjectionRequest {
            language_id: "rust",
            selector,
            query: "render",
            workspace: ".",
        });
        assert!(projection.canonical_selector.is_none(), "{projection:?}");
        assert!(projection.owner_node_id.is_none(), "{projection:?}");
        assert!(projection.selector_node_id.is_none(), "{projection:?}");
        assert!(
            projection
                .nodes
                .iter()
                .all(|node| { node["kind"] != "owner" && node["kind"] != "item" })
        );
        assert!(projection.edges.iter().all(|edge| {
            !matches!(
                edge["relation"].as_str(),
                Some("serves_owner" | "owns_item" | "targets_item")
            )
        }));
    }
}
