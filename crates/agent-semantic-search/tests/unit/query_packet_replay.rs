use serde_json::{Value, json};

use crate::{QueryPacketReplayRequest, query_packet_matches_request, render_query_packet_stdout};

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
    let code_request = code_query_request(
        "src/search/api.rs",
        "render_rust_project_harness_search_view_with_config",
    );

    assert!(query_packet_matches_request(
        &query_packet(
            "src/search/api.rs",
            "render_rust_project_harness_search_view_with_config",
        ),
        request.as_replay_request(),
    ));
    assert!(query_packet_matches_request(
        &query_packet(
            "src/search/api.rs",
            "render_rust_project_harness_search_view_with_config",
        ),
        term_request.as_replay_request(),
    ));
    assert!(!query_packet_matches_request(
        &query_packet(
            "src/cache_cli/writeback.rs",
            "render_rust_project_harness_search_view_with_config",
        ),
        request.as_replay_request(),
    ));
    assert!(!query_packet_matches_request(
        &query_packet(
            "src/search/api.rs",
            "write_prompt_output_cache_after_provider_success",
        ),
        request.as_replay_request(),
    ));
    assert!(!query_packet_matches_request(
        &query_packet(
            "src/search/api.rs",
            "render_rust_project_harness_search_view_with_config",
        ),
        code_request.as_replay_request(),
    ));
}

#[test]
fn query_packet_replay_renders_compact_owner_items() {
    let output = render_query_packet_stdout(&json!({
        "schemaId": "agent.semantic-protocols.semantic-query-packet",
        "method": "query/owner-items",
        "ownerPath": "src/search/api.rs",
        "packageName": "rs-harness",
        "query": "render",
        "outputMode": "seeds",
        "matchMode": "substring",
        "matches": [{
            "name": "render_search",
            "kind": "function",
            "location": {
                "path": "src/search/api.rs",
                "lineRange": "10:20"
            }
        }]
    }))
    .expect("query packet stdout");

    assert!(output.starts_with(
        "[search-owner] q=src/search/api.rs pkg=rs-harness own=1 item=1 itemQuery=render\n"
    ));
    assert!(output.contains(
        "|item render_search kind=function next=symbol:render_search read=src/search/api.rs:10:20\n"
    ));
}

#[test]
fn query_packet_replay_rejects_code_output_mode() {
    assert!(
        render_query_packet_stdout(&json!({
            "schemaId": "agent.semantic-protocols.semantic-query-packet",
            "method": "query/owner-items",
            "ownerPath": "src/search/api.rs",
            "query": "render",
            "outputMode": "code",
            "matches": []
        }))
        .is_none()
    );
}

struct ReplayRequest {
    is_query_method: bool,
    forwarded_args: Vec<String>,
}

impl ReplayRequest {
    fn as_replay_request(&self) -> QueryPacketReplayRequest<'_> {
        QueryPacketReplayRequest {
            is_query_method: self.is_query_method,
            forwarded_args: &self.forwarded_args,
        }
    }
}

fn query_request(owner_path: &str, query: &str) -> ReplayRequest {
    request_with_query_flag(owner_path, "--query", query)
}

fn term_request(owner_path: &str, query: &str) -> ReplayRequest {
    request_with_query_flag(owner_path, "--term", query)
}

fn code_query_request(owner_path: &str, query: &str) -> ReplayRequest {
    let mut request = request_with_query_flag(owner_path, "--query", query);
    request.forwarded_args.insert(3, "--code".to_string());
    request
}

fn request_with_query_flag(owner_path: &str, flag: &str, query: &str) -> ReplayRequest {
    ReplayRequest {
        is_query_method: true,
        forwarded_args: vec![
            owner_path.to_string(),
            flag.to_string(),
            query.to_string(),
            ".".to_string(),
        ],
    }
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
