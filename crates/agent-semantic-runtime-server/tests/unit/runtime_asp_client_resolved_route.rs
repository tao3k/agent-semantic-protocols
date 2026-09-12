// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use super::{
    selected_playbook_provider_targets, selected_provider_targets,
    workspace_search_materialization_key,
};

fn provider(language: &str) -> agent_semantic_search::WorkspaceSearchProvider {
    agent_semantic_search::WorkspaceSearchProvider {
        language_id: language.to_owned(),
        provider_id: format!("asp-{language}"),
        source_extensions: vec![],
        search_supported: true,
        producer_axes: vec![agent_semantic_search::WorkspaceSearchProducerAxis::Language],
        enhanced_query_capability: None,
    }
}

#[test]
fn pipe_order_is_preserved_and_duplicates_are_removed() {
    let installed = vec![
        ("python".to_owned(), "asp-python".to_owned()),
        ("rust".to_owned(), "asp-rust".to_owned()),
    ];
    let targets = selected_provider_targets(Some("rust|python|rust"), &installed)
        .unwrap_or_else(|_| panic!("selected targets"));

    assert_eq!(
        targets
            .iter()
            .map(|target| target.language_id.as_str())
            .collect::<Vec<_>>(),
        vec!["rust", "python"]
    );
}

#[test]
fn unrelated_uninstalled_provider_is_not_part_of_selected_closure() {
    let installed = vec![("rust".to_owned(), "asp-rust".to_owned())];
    let targets = selected_provider_targets(Some("rust"), &installed)
        .unwrap_or_else(|_| panic!("selected target"));

    assert_eq!(targets.len(), 1);
    assert_eq!(targets[0].provider_id.as_deref(), Some("asp-rust"));
}

#[test]
fn playbook_language_set_is_admitted_before_generation_work() {
    let providers = vec![provider("org")];
    assert!(selected_playbook_provider_targets(Some("org"), None, &providers).is_ok());
    let error = selected_playbook_provider_targets(Some("rust"), None, &providers)
        .expect_err("an uninstalled producer cannot enter generation work");
    let super::AspClientOperationError::Message(message) = error else {
        panic!("producer rejection must remain a local typed message")
    };
    assert!(message.contains("producer is not installed"), "{message}");
}

fn search_request(
    pattern: &str,
) -> agent_semantic_client_protocol::AspClientWorkspaceSearchPlaybookRequest {
    agent_semantic_client_protocol::AspClientWorkspaceSearchPlaybookRequest {
        schema_id: "agent.semantic-protocols.asp-client-workspace-search-playbook-request"
            .to_owned(),
        schema_version: "1".to_owned(),
        language: Some("rust".to_owned()),
        documents: None,
        workspace: None,
        rg: Some(vec![vec![
            "-n".to_owned(),
            pattern.to_owned(),
            ".".to_owned(),
        ]]),
        tantivy: Some(vec![vec!["body:runtime".to_owned()]]),
        syntax: None,
        native_syntax: None,
        graph: None,
        clause_order: vec![
            agent_semantic_client_protocol::AspClientSearchPlaybookClauseRef {
                axis: agent_semantic_client_protocol::AspClientSearchPlaybookClauseAxis::Rg,
                block_index: 0,
            },
            agent_semantic_client_protocol::AspClientSearchPlaybookClauseRef {
                axis: agent_semantic_client_protocol::AspClientSearchPlaybookClauseAxis::Tantivy,
                block_index: 0,
            },
        ],
    }
}

#[test]
fn search_materialization_identity_binds_generation_and_normalized_request() {
    let request = search_request("ready");
    let first = workspace_search_materialization_key(&request, "blake3-256:generation-a")
        .unwrap_or_else(|_| panic!("first materialization identity"));
    let repeated = workspace_search_materialization_key(&request, "blake3-256:generation-a")
        .unwrap_or_else(|_| panic!("repeated materialization identity"));
    let next_generation = workspace_search_materialization_key(&request, "blake3-256:generation-b")
        .unwrap_or_else(|_| panic!("next-generation materialization identity"));
    let next_request = workspace_search_materialization_key(
        &search_request("different"),
        "blake3-256:generation-a",
    )
    .unwrap_or_else(|_| panic!("next-request materialization identity"));

    assert_eq!(first, repeated);
    assert_ne!(first, next_generation);
    assert_ne!(first, next_request);
}
// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later
