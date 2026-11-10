// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use agent_semantic_client_protocol::EnhancedQueryCapabilityTable;

use super::syntax_plan_context;

const GENERATION_DIGEST: &str =
    "blake3-256:1111111111111111111111111111111111111111111111111111111111111111";

fn provider() -> agent_semantic_search::WorkspaceSearchProvider {
    let capability: EnhancedQueryCapabilityTable = serde_json::from_str(include_str!(
        "../../../../languages/asp-rust/tree-sitter/tree-sitter-rust/enhanced-query-capabilities.v1.json"
    ))
    .expect("Rust enhanced Query capability table");
    agent_semantic_search::WorkspaceSearchProvider {
        language_id: "rust".to_owned(),
        provider_id: "asp-rust".to_owned(),
        source_extensions: vec!["rs".to_owned()],
        search_supported: true,
        producer_axes: vec![agent_semantic_search::WorkspaceSearchProducerAxis::Language],
        enhanced_query_capability: Some(capability),
    }
}

#[test]
fn context_returns_only_the_resident_generation_and_capability() {
    let response = syntax_plan_context("rust", GENERATION_DIGEST, &[provider()])
        .unwrap_or_else(|_| panic!("resident syntax plan context"));
    assert_eq!(response.generation_digest, GENERATION_DIGEST);
    assert_eq!(response.capability.language_id, "rust");
    assert_eq!(response.capability.provider_id, "asp-rust");
    assert!(response.capability.validate().is_ok());
}

#[test]
fn context_rejects_a_provider_without_an_enhanced_query_capability() {
    let mut unavailable = provider();
    unavailable.enhanced_query_capability = None;
    let error = syntax_plan_context("rust", GENERATION_DIGEST, &[unavailable])
        .expect_err("capability absence must fail closed");
    let super::AspClientOperationError::Message(message) = error else {
        panic!("expected typed message")
    };
    assert!(message.contains("capability-table-missing"), "{message}");
}
