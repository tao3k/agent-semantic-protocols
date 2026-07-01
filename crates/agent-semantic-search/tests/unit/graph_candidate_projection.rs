use crate::{
    GraphProjectionCandidate, graph_candidate_hot_nodes, graph_candidate_item_node_id,
    graph_candidate_item_nodes,
};

#[test]
fn graph_candidate_item_nodes_use_structural_identity_without_line_range_selectors() {
    let candidate = GraphProjectionCandidate::new(
        "src/lib.rs",
        3,
        4,
        "SearchOwner",
        "pub fn SearchOwner() {}",
        "source-index",
        "high",
    );

    let nodes = graph_candidate_item_nodes("rust", std::slice::from_ref(&candidate), 8);

    assert_eq!(nodes.len(), 1);
    assert_eq!(nodes[0]["id"], graph_candidate_item_node_id(&candidate));
    assert_eq!(
        nodes[0]["structuralSelector"],
        "rust://src/lib.rs#item/symbol/SearchOwner"
    );
    assert_eq!(nodes[0]["displayLineRange"], "3:4");
    assert!(
        !nodes[0]["structuralSelector"]
            .as_str()
            .expect("structural selector")
            .contains(":3:")
    );
    assert_eq!(
        nodes[0]["syntaxQuery"].as_str().expect("syntax query"),
        "((function_item name: (_) @function.name) (#eq? @function.name \"SearchOwner\"))"
    );
}

#[test]
fn graph_candidate_hot_nodes_keep_code_policy_and_context_window() {
    let candidate = GraphProjectionCandidate::new(
        "src/lib.rs",
        3,
        4,
        "SearchOwner",
        "pub fn SearchOwner() {}",
        "dynamic-overlay",
        "medium",
    );

    let nodes = graph_candidate_hot_nodes("rust", &[candidate], 8);

    assert_eq!(nodes.len(), 1);
    assert_eq!(nodes[0]["kind"], "hot");
    assert_eq!(nodes[0]["action"], "code");
    assert_eq!(nodes[0]["startLine"], 1);
    assert_eq!(nodes[0]["endLine"], 15);
    assert_eq!(nodes[0]["codePolicy"], "requires-exact-code");
    assert_eq!(
        nodes[0]["structuralSelector"],
        "rust://src/lib.rs#range/hot/SearchOwner"
    );
}
