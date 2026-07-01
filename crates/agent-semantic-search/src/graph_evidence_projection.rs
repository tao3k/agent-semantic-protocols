use std::collections::HashMap;

pub fn graph_frontier_has_only_owner_or_topology_nodes(kinds: &HashMap<String, String>) -> bool {
    !kinds.is_empty()
        && kinds.values().all(|kind| {
            matches!(
                kind.as_str(),
                "owner" | "workspace" | "provider-root" | "submodule"
            )
        })
}
