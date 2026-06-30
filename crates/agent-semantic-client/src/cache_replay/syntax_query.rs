//! `semantic-tree-sitter-query` structured replay rendering facade.

use agent_semantic_client_db::ClientDbSyntaxQueryReplay;
use agent_semantic_search::{
    SyntaxQueryReplayCapture, SyntaxQueryRowsReplay,
    render_semantic_tree_sitter_query_rows_stdout as render_search_syntax_rows_stdout,
    render_semantic_tree_sitter_query_stdout as render_search_syntax_packet_stdout,
};
use serde_json::Value;

pub(crate) fn render_semantic_tree_sitter_query_stdout(packet: &Value) -> Option<String> {
    render_search_syntax_packet_stdout(packet)
}

pub(crate) fn render_semantic_tree_sitter_query_rows_stdout(
    replay: &ClientDbSyntaxQueryReplay,
) -> String {
    render_search_syntax_rows_stdout(SyntaxQueryRowsReplay {
        language_id: replay.language_id.as_str(),
        input_form: &replay.input_form,
        input_kind: replay.input_kind.as_str(),
        grammar_id: &replay.grammar_id,
        grammar_profile_version: &replay.grammar_profile_version,
        compiled_source: &replay.compiled_source,
        captures: &replay.captures,
        rows: replay
            .rows
            .iter()
            .map(|row| SyntaxQueryReplayCapture {
                match_locator: row.match_locator.clone(),
                capture_locator: row.capture_locator.clone(),
                capture_name: row.capture_name.clone(),
                capture_node_type: Some(row.capture_node_type.as_str().to_string()),
                item_node_type: Some(row.item_node_type.as_str().to_string()),
                field: row.field.clone(),
                text: row.text.clone(),
            })
            .collect(),
    })
}
