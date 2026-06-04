use crate::cache_replay::query_packet_matches_request;
use agent_semantic_client_core::{ClientMethod, ClientRequest};
use serde_json::{Value, json};

#[test]
fn query_packet_replay_requires_matching_owner_and_query() {
    let request = query_request(
        "src/search/api.rs",
        "render_rust_project_harness_search_view_with_config",
    );
    let term_request = term_request(
        "src/search/api.rs",
        "render_rust_project_harness_search_view_with_config",
    );

    assert!(
        query_packet_matches_request(
            &query_packet(
                "src/search/api.rs",
                "render_rust_project_harness_search_view_with_config",
            ),
            &request,
        )
        .is_some()
    );
    assert!(
        query_packet_matches_request(
            &query_packet(
                "src/search/api.rs",
                "render_rust_project_harness_search_view_with_config",
            ),
            &term_request,
        )
        .is_some()
    );
    assert!(
        query_packet_matches_request(
            &query_packet(
                "src/cache_cli/writeback.rs",
                "render_rust_project_harness_search_view_with_config",
            ),
            &request,
        )
        .is_none()
    );
    assert!(
        query_packet_matches_request(
            &query_packet(
                "src/search/api.rs",
                "write_prompt_output_cache_after_provider_success"
            ),
            &request,
        )
        .is_none()
    );
}

fn query_request(owner_path: &str, query: &str) -> ClientRequest {
    request_with_query_flag(owner_path, "--query", query)
}

fn term_request(owner_path: &str, query: &str) -> ClientRequest {
    request_with_query_flag(owner_path, "--term", query)
}

fn request_with_query_flag(owner_path: &str, flag: &str, query: &str) -> ClientRequest {
    ClientRequest::new(ClientMethod::Query, ".").with_forwarded_args(vec![
        owner_path.to_string(),
        flag.to_string(),
        query.to_string(),
        "--code".to_string(),
        ".".to_string(),
    ])
}

fn query_packet(owner_path: &str, query: &str) -> Value {
    json!({
        "schemaId": "agent.semantic-protocols.semantic-query-packet",
        "method": "query/owner-items",
        "ownerPath": owner_path,
        "query": query,
        "matches": []
    })
}
