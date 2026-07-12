use serde_json::json;

use super::semantic_tree_sitter_query_execution_is_complete;

#[test]
fn semantic_tree_sitter_query_replay_requires_execution_facts() {
    assert!(!semantic_tree_sitter_query_execution_is_complete(
        &json!({})
    ));
    assert!(!semantic_tree_sitter_query_execution_is_complete(&json!({
        "execution": {"engine": "tree-sitter-querycursor"}
    })));
    assert!(!semantic_tree_sitter_query_execution_is_complete(&json!({
        "execution": {
            "engine": "native-parser-projection",
            "predicateEvaluator": "asp-tree-sitter-predicate-v1",
            "matchStatus": "hit"
        }
    })));
    assert!(semantic_tree_sitter_query_execution_is_complete(&json!({
        "execution": {
            "engine": "tree-sitter-querycursor",
            "predicateEvaluator": "asp-tree-sitter-predicate-v1",
            "matchStatus": "hit"
        }
    })));
}
