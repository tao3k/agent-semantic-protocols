//! Shared deterministic node identity for selector-side search metadata.

pub(super) fn stable_node_id(kind: &str, value: &str) -> String {
    agent_semantic_search::stable_graph_node_id(kind, value)
}
