use std::collections::HashMap;

use crate::graph_frontier_has_only_owner_or_topology_nodes;

#[test]
fn graph_frontier_owner_topology_predicate_accepts_only_route_nodes() {
    let mut kinds = HashMap::new();
    kinds.insert("owner:src/lib.rs".to_string(), "owner".to_string());
    kinds.insert("workspace:root".to_string(), "workspace".to_string());
    kinds.insert("provider:rust".to_string(), "provider-root".to_string());
    kinds.insert(
        "submodule:languages/rust".to_string(),
        "submodule".to_string(),
    );

    assert!(graph_frontier_has_only_owner_or_topology_nodes(&kinds));
}

#[test]
fn graph_frontier_owner_topology_predicate_rejects_empty_or_item_nodes() {
    assert!(!graph_frontier_has_only_owner_or_topology_nodes(
        &HashMap::new()
    ));

    let mut kinds = HashMap::new();
    kinds.insert("owner:src/lib.rs".to_string(), "owner".to_string());
    kinds.insert("item:src/lib.rs-search".to_string(), "item".to_string());

    assert!(!graph_frontier_has_only_owner_or_topology_nodes(&kinds));
}
