use crate::cache_replay::{
    query_packet_matches_request, search_fzf_packet_matches_request,
    search_output_artifact_replay_safe,
};
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
    let code_request = code_query_request(
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
    assert!(
        query_packet_matches_request(
            &query_packet(
                "src/search/api.rs",
                "render_rust_project_harness_search_view_with_config",
            ),
            &code_request,
        )
        .is_none()
    );
}

#[test]
fn search_fzf_packet_replay_requires_matching_query() {
    let request = fzf_request("cache replay");
    let equals_request = fzf_equals_request("cache replay");
    let code_request = fzf_code_request("cache replay");

    assert!(search_fzf_packet_matches_request(&fzf_packet("cache replay"), &request).is_some());
    assert!(
        search_fzf_packet_matches_request(&fzf_packet("cache replay"), &equals_request).is_some()
    );
    assert!(search_fzf_packet_matches_request(&fzf_packet("cache"), &request).is_none());
    assert!(
        search_fzf_packet_matches_request(&fzf_packet("cache replay"), &code_request).is_none()
    );
}

#[test]
fn search_packet_replay_accepts_graph_turbo_frontier() {
    let stdout = "\
[graph-frontier] profile=owner-query alg=typed-ppr-diverse seed=Q budget=10\n\
legend: ID=kind:role(value)!next; edge SRC>{DST:rel}; frontier ID.next\n\
aliases=G:graph,Q:query\n\
rank=Q frontier=Q.fzf\n";

    assert!(search_output_artifact_replay_safe(stdout.as_bytes()));
}

#[test]
fn search_packet_replay_rejects_obsolete_search_frontier() {
    let stdout = "\
[search-fzf] q=cache view=hits\n\
legend: ID=kind:role(value)!next; edge SRC>{DST:rel}; frontier ID.next\n\
aliases: graph:{G=search,Q=query}\n\
rank=Q frontier=Q.fzf\n";

    assert!(!search_output_artifact_replay_safe(stdout.as_bytes()));
}

fn query_request(owner_path: &str, query: &str) -> ClientRequest {
    request_with_query_flag(owner_path, "--query", query)
}

fn term_request(owner_path: &str, query: &str) -> ClientRequest {
    request_with_query_flag(owner_path, "--term", query)
}

fn code_query_request(owner_path: &str, query: &str) -> ClientRequest {
    let mut request = request_with_query_flag(owner_path, "--query", query);
    request.forwarded_args.insert(3, "--code".to_string());
    request
}

fn request_with_query_flag(owner_path: &str, flag: &str, query: &str) -> ClientRequest {
    ClientRequest::new(ClientMethod::Query, ".").with_forwarded_args(vec![
        owner_path.to_string(),
        flag.to_string(),
        query.to_string(),
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

fn fzf_request(query: &str) -> ClientRequest {
    ClientRequest::new(ClientMethod::Search, ".").with_forwarded_args(vec![
        "fzf".to_string(),
        query.to_string(),
        "owner".to_string(),
        "tests".to_string(),
        "--view".to_string(),
        "seeds".to_string(),
        ".".to_string(),
    ])
}

fn fzf_equals_request(query: &str) -> ClientRequest {
    ClientRequest::new(ClientMethod::Search, ".").with_forwarded_args(vec![
        "fzf".to_string(),
        query.to_string(),
        "--view=seeds".to_string(),
        ".".to_string(),
    ])
}

fn fzf_code_request(query: &str) -> ClientRequest {
    let mut request = fzf_request(query);
    request.forwarded_args.insert(2, "--code".to_string());
    request
}

fn fzf_packet(query: &str) -> Value {
    json!({
        "schemaId": "agent.semantic-protocols.semantic-search-packet",
        "method": "search/fzf",
        "query": query,
        "nodes": []
    })
}
