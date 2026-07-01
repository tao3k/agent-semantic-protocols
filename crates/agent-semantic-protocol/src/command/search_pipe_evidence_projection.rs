//! Evidence-kind predicates facade for compact search-pipe projection.

use std::collections::HashMap;

pub(super) fn rank_frontier_has_only_owner_or_topology_nodes(
    kinds: &HashMap<String, String>,
) -> bool {
    agent_semantic_search::graph_frontier_has_only_owner_or_topology_nodes(kinds)
}
