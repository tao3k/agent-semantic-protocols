// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use agent_semantic_search::{
    GraphNativeBlock, NormalizedWorkspaceSearchPlaybookRequest, SearchPlaybookClauseAxis,
    SearchPlaybookClauseRef, WorkspaceSearchPlanBinding, WorkspaceSearchProducerAxis,
    WorkspaceSearchProvider, build_workspace_search_playbook_plan,
};
use serde_json::json;

fn provider(language: &str, axis: WorkspaceSearchProducerAxis) -> WorkspaceSearchProvider {
    WorkspaceSearchProvider {
        language_id: language.to_owned(),
        provider_id: format!("asp-{language}"),
        source_extensions: vec![if language == "rust" { "rs" } else { "py" }.to_owned()],
        search_supported: true,
        producer_axes: vec![axis],
        enhanced_query_capability: None,
    }
}

fn syntax_plan() -> agent_semantic_client_protocol::ResidentSyntaxQueryPlan {
    let digest = format!("blake3-256:{}", "1".repeat(64));
    serde_json::from_value(json!({
        "schemaId": "agent.semantic-protocols.resident-syntax-query-plan",
        "schemaVersion": "1",
        "profileId": "asp.enhanced-tree-sitter-query.v1",
        "planDigest": digest,
        "queryDigest": digest,
        "languageId": "rust",
        "providerId": "asp-rust",
        "parserAbiDigest": digest,
        "queryGrammarDigest": digest,
        "operatorTableDigest": digest,
        "capabilityTableDigest": digest,
        "generationDigest": digest,
        "patterns": [{
            "index": 0,
            "captures": [{
                "name": "item",
                "residentFactPath": "selector",
                "cardinality": {"minimum": 1, "maximum": 1},
                "capabilityRowId": "rust.capture.item"
            }],
            "structure": {"kind": "true", "origin": {
                "kind": "capture", "capabilityRowId": "rust.capture.item"
            }},
            "predicates": []
        }],
        "selectedFields": ["selector"],
        "requiredCapabilityRows": ["rust.capture.item"],
        "regexPrograms": []
    }))
    .expect("resident syntax plan fixture")
}

#[test]
fn document_producers_use_the_documents_selector() {
    let mut document_request = request();
    document_request.language = None;
    document_request.documents = Some("org".to_owned());
    let document_provider = provider("org", WorkspaceSearchProducerAxis::Document);
    assert!(
        build_workspace_search_playbook_plan(
            &document_request,
            WorkspaceSearchPlanBinding {
                project_id: "project".to_owned(),
                workspace_id: "workspace".to_owned(),
                content_generation_digest: "generation".to_owned(),
            },
            [document_provider],
        )
        .is_ok()
    );
}

#[test]
fn document_producer_is_rejected_on_the_language_axis() {
    let mut document_request = request();
    document_request.language = Some("org".to_owned());
    document_request.documents = None;
    let error = build_workspace_search_playbook_plan(
        &document_request,
        WorkspaceSearchPlanBinding {
            project_id: "project".to_owned(),
            workspace_id: "workspace".to_owned(),
            content_generation_digest: "generation".to_owned(),
        },
        [provider("org", WorkspaceSearchProducerAxis::Document)],
    )
    .expect_err("Org is a document producer, not a programming language");
    assert!(error.contains("wrong axis"), "{error}");
}

fn request() -> NormalizedWorkspaceSearchPlaybookRequest {
    NormalizedWorkspaceSearchPlaybookRequest {
        language: Some("rust|python".to_owned()),
        documents: None,
        workspace: None,
        rg: vec![vec![
            "-n".to_owned(),
            "owner|consumer".to_owned(),
            ".".to_owned(),
        ]],
        tantivy: vec![vec![
            "title:\"authority owner\"^2 OR body:impact".to_owned(),
        ]],
        syntax: vec![
            agent_semantic_client_protocol::AspClientSearchPlaybookSyntaxBlock {
                producer: "rust".to_owned(),
                plan: syntax_plan(),
            },
        ],
        native_syntax: vec!["rust://src/registry.rs#item/implementation/type/Registry".to_owned()],
        graph: vec![GraphNativeBlock {
            language: "pgql".to_owned(),
            argv: vec!["MATCH (a)-[r]->(b) RETURN a, r, b".to_owned()],
        }],
        clause_order: vec![
            SearchPlaybookClauseRef {
                axis: SearchPlaybookClauseAxis::Syntax,
                block_index: 0,
            },
            SearchPlaybookClauseRef {
                axis: SearchPlaybookClauseAxis::NativeSyntax,
                block_index: 0,
            },
            SearchPlaybookClauseRef {
                axis: SearchPlaybookClauseAxis::Rg,
                block_index: 0,
            },
            SearchPlaybookClauseRef {
                axis: SearchPlaybookClauseAxis::Tantivy,
                block_index: 0,
            },
            SearchPlaybookClauseRef {
                axis: SearchPlaybookClauseAxis::Graph,
                block_index: 0,
            },
        ],
    }
}

#[test]
fn plan_preserves_agent_authored_clause_priority_and_graph_barrier() {
    let plan = build_workspace_search_playbook_plan(
        &request(),
        WorkspaceSearchPlanBinding {
            project_id: "project".to_owned(),
            workspace_id: "workspace".to_owned(),
            content_generation_digest: "generation".to_owned(),
        },
        [
            provider("rust", WorkspaceSearchProducerAxis::Language),
            provider("python", WorkspaceSearchProducerAxis::Language),
        ],
    )
    .expect("progressive plan");
    assert_eq!(plan.routes[0].language_id, "python");
    assert_eq!(plan.routes[1].language_id, "rust");
    assert_eq!(
        plan.axes.clause_order,
        vec![
            SearchPlaybookClauseRef {
                axis: SearchPlaybookClauseAxis::Syntax,
                block_index: 0,
            },
            SearchPlaybookClauseRef {
                axis: SearchPlaybookClauseAxis::NativeSyntax,
                block_index: 0,
            },
            SearchPlaybookClauseRef {
                axis: SearchPlaybookClauseAxis::Rg,
                block_index: 0,
            },
            SearchPlaybookClauseRef {
                axis: SearchPlaybookClauseAxis::Tantivy,
                block_index: 0,
            },
            SearchPlaybookClauseRef {
                axis: SearchPlaybookClauseAxis::Graph,
                block_index: 0,
            },
        ]
    );
    assert_eq!(plan.axes.rg[0][0], "-n");
}

#[test]
fn request_without_a_producer_axis_is_rejected() {
    let error = build_workspace_search_playbook_plan(
        &NormalizedWorkspaceSearchPlaybookRequest {
            language: None,
            documents: None,
            workspace: None,
            rg: vec![vec!["owner".to_owned(), ".".to_owned()]],
            tantivy: vec![vec!["title:\"owner route\"^2 OR body:consumer".to_owned()]],
            syntax: vec![],
            native_syntax: vec![],
            graph: vec![],
            clause_order: vec![
                SearchPlaybookClauseRef {
                    axis: SearchPlaybookClauseAxis::Rg,
                    block_index: 0,
                },
                SearchPlaybookClauseRef {
                    axis: SearchPlaybookClauseAxis::Tantivy,
                    block_index: 0,
                },
            ],
        },
        WorkspaceSearchPlanBinding {
            project_id: "project".to_owned(),
            workspace_id: "workspace".to_owned(),
            content_generation_digest: "generation".to_owned(),
        },
        [
            provider("rust", WorkspaceSearchProducerAxis::Language),
            provider("python", WorkspaceSearchProducerAxis::Language),
        ],
    )
    .expect_err("a producer axis is required");
    assert!(
        error.contains("requires --language or --documents"),
        "{error}"
    );
}

#[test]
fn explicit_registered_workspace_must_equal_the_runtime_binding() {
    let mut request = request();
    request.workspace = Some("other-workspace".to_owned());
    let error = build_workspace_search_playbook_plan(
        &request,
        WorkspaceSearchPlanBinding {
            project_id: "project".to_owned(),
            workspace_id: "workspace".to_owned(),
            content_generation_digest: "generation".to_owned(),
        },
        [
            provider("rust", WorkspaceSearchProducerAxis::Language),
            provider("python", WorkspaceSearchProducerAxis::Language),
        ],
    )
    .expect_err("request identity cannot differ from the Runtime-bound workspace");
    assert!(
        error.contains("registered workspace binding mismatch"),
        "error={error}"
    );
}
