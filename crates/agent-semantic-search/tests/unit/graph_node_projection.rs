use crate::graph_node_projection::{owner_path_graph_nodes, stable_graph_node_id};

#[test]
fn stable_graph_node_id_normalizes_owner_paths() {
    assert_eq!(
        stable_graph_node_id("owner", "Src/Generated Lib.rs"),
        "owner:src/generated-lib.rs"
    );
    assert_eq!(stable_graph_node_id("owner", "!!!"), "owner:node");
}

#[test]
fn owner_path_graph_nodes_project_owner_nodes() {
    let nodes = owner_path_graph_nodes(&["src/lib.rs".to_string()]);

    assert_eq!(nodes.len(), 1);
    assert_eq!(nodes[0]["id"], "owner:src/lib.rs");
    assert_eq!(nodes[0]["kind"], "owner");
    assert_eq!(nodes[0]["role"], "path");
    assert_eq!(nodes[0]["action"], "owner");
    assert_eq!(nodes[0]["path"], "src/lib.rs");
}
