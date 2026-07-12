//! `semantic-tree-sitter-query` structured replay rendering facade.

use agent_semantic_search::render_semantic_tree_sitter_query_stdout as render_search_syntax_packet_stdout;
use serde_json::Value;

pub(crate) fn render_semantic_tree_sitter_query_stdout(packet: &Value) -> Option<String> {
    render_search_syntax_packet_stdout(packet)
}
