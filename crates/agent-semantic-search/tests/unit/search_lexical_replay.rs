use serde_json::json;

use crate::{SearchLexicalReplayRequest, search_lexical_packet_matches_request};

#[test]
fn lexical_replay_matches_schema_method_and_query() {
    let args = vec!["lexical".to_string(), "needle".to_string()];
    let packet = json!({
        "schemaId": "agent.semantic-protocols.semantic-search-packet",
        "method": "search/lexical",
        "query": "needle",
    });

    assert!(search_lexical_packet_matches_request(
        &packet,
        SearchLexicalReplayRequest {
            is_search_method: true,
            forwarded_args: &args,
        }
    ));
}

#[test]
fn lexical_replay_rejects_json_and_code_outputs() {
    let packet = json!({
        "schemaId": "agent.semantic-protocols.semantic-search-packet",
        "method": "search/lexical",
        "query": "needle",
    });

    for forbidden in ["--json", "--code"] {
        let args = vec![
            "lexical".to_string(),
            "needle".to_string(),
            forbidden.to_string(),
        ];
        assert!(!search_lexical_packet_matches_request(
            &packet,
            SearchLexicalReplayRequest {
                is_search_method: true,
                forwarded_args: &args,
            }
        ));
    }
}

#[test]
fn lexical_replay_rejects_non_search_requests() {
    let args = vec!["lexical".to_string(), "needle".to_string()];
    let packet = json!({
        "schemaId": "agent.semantic-protocols.semantic-search-packet",
        "method": "search/lexical",
        "query": "needle",
    });

    assert!(!search_lexical_packet_matches_request(
        &packet,
        SearchLexicalReplayRequest {
            is_search_method: false,
            forwarded_args: &args,
        }
    ));
}
