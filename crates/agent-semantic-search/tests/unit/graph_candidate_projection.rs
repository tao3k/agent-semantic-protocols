use crate::graph_candidate_projection::{
    GraphCandidateHotNodesRequest, GraphCandidateItemNodesRequest, GraphProjectionCandidate,
    graph_candidate_hot_nodes, graph_candidate_item_node_id, graph_candidate_item_nodes,
    graph_projection_candidate_readiness,
};

#[test]
fn graph_candidate_item_nodes_use_registered_language_structural_identity_without_line_ranges() {
    let candidate = GraphProjectionCandidate::new(
        "src/lib.rs",
        3,
        4,
        "SearchOwner",
        "pub fn SearchOwner() {}",
        "source-index",
        "high",
    );
    let language_ids = agent_semantic_hook::registered_language_ids();

    assert!(!language_ids.is_empty());

    for language_id in language_ids {
        let nodes = graph_candidate_item_nodes(GraphCandidateItemNodesRequest::new(
            language_id.as_str(),
            std::slice::from_ref(&candidate),
            8,
        ));

        assert_eq!(nodes.len(), 1, "language_id={language_id}");
        assert_eq!(nodes[0]["id"], graph_candidate_item_node_id(&candidate));
        assert_eq!(
            nodes[0]["structuralSelector"],
            format!("{language_id}://src/lib.rs#item/symbol/SearchOwner")
        );
        assert_eq!(nodes[0]["displayLineRange"], "3:4");
        assert!(
            !nodes[0]["structuralSelector"]
                .as_str()
                .expect("structural selector")
                .contains(":3:"),
            "language_id={language_id}"
        );
        assert_eq!(nodes[0]["candidateState"], "selector-ready");
        assert_eq!(nodes[0]["rankEligible"], true);
        assert_eq!(nodes[0]["fields"]["candidateState"], "selector-ready");
        assert_eq!(nodes[0]["fields"]["rankEligible"], true);
    }
}

#[test]
fn graph_projection_candidate_readiness_marks_path_candidates_inventory_only() {
    let candidate = GraphProjectionCandidate::new(
        "src/lib.rs",
        3,
        4,
        "SearchOwner",
        "src/lib.rs",
        "finder-path",
        "path",
    );

    let readiness = graph_projection_candidate_readiness(&candidate);

    assert_eq!(readiness.as_str(), "inventory-only");
    assert!(!readiness.rank_eligible());
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

    let nodes =
        graph_candidate_hot_nodes(GraphCandidateHotNodesRequest::new("rust", &[candidate], 8));

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
