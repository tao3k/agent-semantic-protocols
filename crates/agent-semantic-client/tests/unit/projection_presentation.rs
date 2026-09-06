// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-only

use agent_semantic_client::projection_presentation::ProjectionPresentation;
use agent_semantic_client::projection_presentation::render_exact_projection_response;
use agent_semantic_client::projection_presentation::render_search_playbook_contract_response;
use agent_semantic_client::projection_presentation::render_workspace_search_playbook_result;
use agent_semantic_client::projection_presentation::render_workspace_syntax_query_response;
use agent_semantic_client_protocol::ClientFrame;
use serde_json::Value;
use serde_json::json;

fn response(result: Value) -> ClientFrame {
    serde_json::from_value(json!({
        "kind": "response",
        "schemaId": "agent.semantic-protocols.client.frame",
        "schemaVersion": "1",
        "protocolId": "agent.semantic-protocols.client",
        "protocolVersion": "1",
        "sessionId": "session-projection",
        "projectId": "repo-projection",
        "workspaceId": "workspace-projection",
        "requestId": "request-projection",
        "outcome": "ready",
        "result": result,
        "error": null,
        "catalog": null
    }))
    .expect("typed response fixture")
}

#[test]
fn renders_provider_projection_bytes_as_utf8_text() {
    let frame = response(json!({
        "result": {"bytes": [112, 117, 98, 32, 102, 110, 32, 114, 101, 97, 100, 121, 40, 41, 32, 123, 125]}
    }));
    assert_eq!(
        render_exact_projection_response(&frame, ProjectionPresentation::Text).unwrap(),
        "pub fn ready() {}"
    );
}

#[test]
fn rejects_non_byte_projection_values() {
    let frame = response(json!({"result": {"bytes": [256]}}));
    assert_eq!(
        render_exact_projection_response(&frame, ProjectionPresentation::Text).unwrap_err(),
        "exact projection response contains a non-byte value"
    );
}

#[test]
fn does_not_expand_an_owner_repair_packet_as_an_exact_projection() {
    let frame = response(json!({
        "result": {
            "state": "owner-for-repair",
            "owner": {"bytes": [101, 110, 116, 105, 114, 101, 32, 111, 119, 110, 101, 114]}
        }
    }));
    assert_eq!(
        render_exact_projection_response(&frame, ProjectionPresentation::Text).unwrap_err(),
        "exact projection response has no text or byte payload"
    );
}

#[test]
fn preserves_runtime_exact_query_failure_terminal() {
    let frame: ClientFrame = serde_json::from_value(json!({
        "kind": "response",
        "schemaId": "agent.semantic-protocols.client.frame",
        "schemaVersion": "1",
        "protocolId": "agent.semantic-protocols.client",
        "protocolVersion": "1",
        "sessionId": "session-projection",
        "projectId": "repo-projection",
        "workspaceId": "workspace-projection",
        "requestId": "request-projection",
        "outcome": "error",
        "result": null,
        "error": {
            "reasonKind": "projection-missing",
            "message": "exact projection terminal: projection-missing",
            "terminal": {
                "schemaId": "agent.semantic-protocols.asp-client-exact-query-failure",
                "schemaVersion": "1",
                "state": "failed",
                "operationId": "request-projection",
                "projectId": "repo-projection",
                "workspaceId": "workspace-projection",
                "languageId": "rust",
                "providerId": "asp-rust",
                "requestedSelector": "rust://src/lib.rs#item/function/missing",
                "resolvedSelector": "rust://src/lib.rs#item/function/missing",
                "projectionKind": "source",
                "phase": "resident-selector-read",
                "reasonKind": "projection-missing",
                "generationDigest": format!("blake3-256:{}", "a".repeat(64)),
                "rootDigest": "b".repeat(64),
                "residentReadElapsedMicros": 7,
                "serviceElapsedMicros": 3,
                "elapsedMicros": 10,
                "workCounters": {
                    "databaseReadCount": 0,
                    "filesystemReadCount": 0,
                    "providerProcessCount": 0,
                    "schedulerTaskCount": 0,
                    "socketOperationCount": 0
                },
                "details": {"selectorState": "projection-missing"}
            }
        },
        "catalog": null
    }))
    .expect("typed failure frame");

    assert_eq!(
        render_exact_projection_response(&frame, ProjectionPresentation::Text).unwrap_err(),
        "exact projection terminal: projection-missing"
    );
    let machine = render_exact_projection_response(&frame, ProjectionPresentation::MachineJson)
        .expect("explicit JSON preserves the typed terminal");
    assert!(machine.contains("\"reasonKind\":\"projection-missing\""));
    assert!(machine.contains("\"phase\":\"resident-selector-read\""));
    assert!(machine.contains("\"serviceElapsedMicros\":3"));
    assert!(!machine.contains("\"recommendedNext\""));
}

#[test]
fn search_playbook_contract_renders_only_example_and_grammar() {
    let frame = response(json!({
        "schemaId": "agent.semantic-protocols.search-playbook-contract-projection",
        "schemaVersion": "1",
        "result": "ready",
        "requestedProducers": ["rust", "python"],
        "projection": {
            "example": "rust example\npython example",
            "grammar": "rust grammar\npython grammar"
        }
    }));
    let rendered = render_search_playbook_contract_response(&frame).unwrap();
    assert_eq!(
        rendered,
        "Example\nrust example\npython example\n\nGrammar\nrust grammar\npython grammar"
    );
    assert!(!rendered.contains("digest"));
    assert!(!rendered.contains("next"));
}

#[test]
fn workspace_search_result_renders_no_plan_digest_source_or_next() {
    let frame = response(serde_json::json!({
        "schemaId": "agent.semantic-protocols.workspace-search-playbook-result",
        "schemaVersion": "1",
        "result": "relationship-supported",
        "queryGrammar": "asp query --selector <exact-selector> --projection source",
        "evidence": [
            {
                "owner": "src/runtime_search_graph.rs",
                "item": "function/evaluate_python_workspace_playbook_graph",
                "selector": "rust://src/runtime_search_graph.rs#item/function/evaluate_python_workspace_playbook_graph",
                "matchedBy": ["rg:0", "syntax:0", "graph:0"],
                "relation": "syntax-capture:function"
            },
            {
                "owner": "src/workspace_playbook_result.rs",
                "item": "function/synthesize_workspace_search_playbook_result",
                "selector": "rust://src/workspace_playbook_result.rs#item/function/synthesize_workspace_search_playbook_result",
                "matchedBy": ["fd:0", "syntax:0", "graph:0"],
                "relation": "syntax-capture:function"
            }
        ]
    }));
    let rendered = render_workspace_search_playbook_result(&frame).unwrap();
    assert_eq!(
        rendered,
        "[search-result] result=relationship-supported evidence=2\nQueryGrammar: asp query --selector <exact-selector> --projection source\nE1 | owner=src/runtime_search_graph.rs | item=function/evaluate_python_workspace_playbook_graph | selector=rust://src/runtime_search_graph.rs#item/function/evaluate_python_workspace_playbook_graph | matchedBy=rg:0|syntax:0|graph:0 | relation=syntax-capture:function\nE2 | owner=src/workspace_playbook_result.rs | item=function/synthesize_workspace_search_playbook_result | selector=rust://src/workspace_playbook_result.rs#item/function/synthesize_workspace_search_playbook_result | matchedBy=fd:0|syntax:0|graph:0 | relation=syntax-capture:function"
    );
    for forbidden in ["plan=", "digest=", "source=", "Next:", "recommendedNext"] {
        assert!(!rendered.contains(forbidden));
    }
}

#[test]
fn workspace_search_result_rejects_internal_plan_as_success() {
    let frame = response(serde_json::json!({
        "schemaId": "agent.semantic-protocols.workspace-search-playbook-plan",
        "schemaVersion": "1",
        "result": "plan-ready",
        "evidence": []
    }));
    assert!(
        render_workspace_search_playbook_result(&frame)
            .unwrap_err()
            .contains("did not execute")
    );
}

#[test]
fn graph_rank_and_authored_clause_priority_survive_client_rendering() {
    use agent_semantic_search::{
        WorkspaceSearchAxisKind, WorkspaceSearchClauseReceipt, WorkspaceSearchGraphFanIn,
        WorkspaceSearchSyntaxCandidate, synthesize_workspace_search_playbook_result,
    };

    let result = synthesize_workspace_search_playbook_result(
        vec![
            WorkspaceSearchClauseReceipt {
                axis: WorkspaceSearchAxisKind::Rg,
                block_index: 0,
                priority_rank: 0,
                candidate_owners: vec!["src/a.rs".to_owned(), "src/b.rs".to_owned()],
                complete: true,
                coverage_complete: true,
                truncated: false,
            },
            WorkspaceSearchClauseReceipt {
                axis: WorkspaceSearchAxisKind::Syntax,
                block_index: 0,
                priority_rank: 1,
                candidate_owners: vec!["src/a.rs".to_owned(), "src/b.rs".to_owned()],
                complete: true,
                coverage_complete: true,
                truncated: false,
            },
        ],
        vec![
            WorkspaceSearchSyntaxCandidate {
                owner: "src/a.rs".to_owned(),
                selector: "rust://src/a.rs#item/function/a".to_owned(),
                relation: "syntax-capture:function".to_owned(),
            },
            WorkspaceSearchSyntaxCandidate {
                owner: "src/b.rs".to_owned(),
                selector: "rust://src/b.rs#item/function/b".to_owned(),
                relation: "syntax-capture:function".to_owned(),
            },
        ],
        Some(WorkspaceSearchGraphFanIn {
            ranked_candidate_owners: vec!["src/b.rs".to_owned(), "src/a.rs".to_owned()],
            applied_clause_count: 1,
            complete: true,
            truncated: false,
        }),
    )
    .unwrap();
    let rendered =
        render_workspace_search_playbook_result(&response(serde_json::to_value(result).unwrap()))
            .unwrap();
    assert_eq!(
        rendered,
        "[search-result] result=relationship-supported evidence=2\nQueryGrammar: asp query --selector <exact-selector> --projection <callable-skeleton|source>\nE1 | owner=src/b.rs | item=function/b | selector=rust://src/b.rs#item/function/b | matchedBy=rg:0|syntax:0|graph:0 | relation=syntax-capture:function\nE2 | owner=src/a.rs | item=function/a | selector=rust://src/a.rs#item/function/a | matchedBy=rg:0|syntax:0|graph:0 | relation=syntax-capture:function"
    );
}

#[test]
fn workspace_search_result_requires_query_grammar_only_for_nonempty_evidence() {
    let nonempty = response(json!({
        "schemaId": "agent.semantic-protocols.workspace-search-playbook-result",
        "schemaVersion": "1",
        "result": "exact-selector-ready",
        "evidence": [{
            "owner": "src/lib.rs",
            "item": "function/run",
            "selector": "rust://src/lib.rs#item/function/run",
            "matchedBy": ["syntax:0"],
            "relation": "syntax-capture:function"
        }]
    }));
    assert!(
        render_workspace_search_playbook_result(&nonempty)
            .unwrap_err()
            .contains("QueryGrammar")
    );

    let empty_with_grammar = response(json!({
        "schemaId": "agent.semantic-protocols.workspace-search-playbook-result",
        "schemaVersion": "1",
        "result": "no-match",
        "queryGrammar": "asp query --selector <exact-selector> --projection <callable-skeleton|source>",
        "evidence": []
    }));
    assert!(
        render_workspace_search_playbook_result(&empty_with_grammar)
            .unwrap_err()
            .contains("must not expose")
    );
}

#[test]
fn syntax_query_renders_bounded_selector_evidence_without_next_or_source() {
    let frame = response(json!({
        "schemaId": "agent.semantic-protocols.asp-client-workspace-syntax-query-response",
        "schemaVersion": "1",
        "state": "ready",
        "evidence": [{
            "owner": "src/lib.rs",
            "selector": "rust://src/lib.rs#item/function/run",
            "relation": "syntax-capture:function.name"
        }]
    }));
    let rendered =
        render_workspace_syntax_query_response(&frame, ProjectionPresentation::Text).unwrap();
    assert_eq!(
        rendered,
        "[query-result] state=ready evidence=1\nE1 | owner=src/lib.rs | selector=rust://src/lib.rs#item/function/run | relation=syntax-capture:function.name"
    );
    assert!(!rendered.contains("next"));
    assert!(!rendered.contains("source"));
    assert!(!rendered.contains("digest"));
}

#[test]
fn syntax_query_zero_match_is_an_explicit_result() {
    let frame = response(json!({
        "schemaId": "agent.semantic-protocols.asp-client-workspace-syntax-query-response",
        "schemaVersion": "1",
        "state": "ready",
        "evidence": []
    }));
    assert_eq!(
        render_workspace_syntax_query_response(&frame, ProjectionPresentation::Text).unwrap(),
        "[query-result] state=ready evidence=0"
    );
}

#[test]
fn text_and_machine_presentations_share_one_typed_response() {
    let frame = response(json!({
        "result": {"bytes": [102, 110, 32, 109, 97, 105, 110, 40, 41, 32, 123, 125]}
    }));
    assert_eq!(
        render_exact_projection_response(&frame, ProjectionPresentation::Text).unwrap(),
        "fn main() {}"
    );
    let machine =
        render_exact_projection_response(&frame, ProjectionPresentation::MachineJson).unwrap();
    assert!(machine.contains("\"bytes\":[102,110,32,109,97,105,110"));
}
