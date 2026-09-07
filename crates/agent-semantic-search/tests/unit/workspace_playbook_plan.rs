// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use agent_semantic_search::{
    GraphNativeBlock, ProducerNativeBlock, ProgressiveSearchPlaybookRequest,
    SearchPlaybookClauseAxis, SearchPlaybookClauseRef, WorkspaceSearchPlanBinding,
    WorkspaceSearchProvider, build_workspace_search_playbook_plan,
};

fn provider(language: &str) -> WorkspaceSearchProvider {
    WorkspaceSearchProvider {
        language_id: language.to_owned(),
        provider_id: format!("asp-{language}"),
        source_extensions: vec![if language == "rust" { "rs" } else { "py" }.to_owned()],
        search_supported: true,
    }
}

fn request() -> ProgressiveSearchPlaybookRequest {
    ProgressiveSearchPlaybookRequest {
        languages: Some("rust|python".to_owned()),
        documents: None,
        workspace: None,
        fd: vec![vec!["-t".to_owned(), "f".to_owned(), ".".to_owned()]],
        rg: vec![vec![
            "-n".to_owned(),
            "owner|consumer".to_owned(),
            ".".to_owned(),
        ]],
        tantivy: vec![vec!["authority|impact".to_owned()]],
        syntax: vec![ProducerNativeBlock {
            producer: "rust".to_owned(),
            argv: vec![
                "--treesitter-query".to_owned(),
                "((identifier) @id)".to_owned(),
            ],
        }],
        native_syntax: vec!["rust://src/registry.rs#item/implementation/type/Registry".to_owned()],
        graph: vec![GraphNativeBlock {
            language: "pgql".to_owned(),
            argv: vec!["MATCH (a)-[r]->(b) RETURN a, r, b".to_owned()],
        }],
        clause_order: vec![
            SearchPlaybookClauseRef {
                axis: SearchPlaybookClauseAxis::Fd,
                block_index: 0,
            },
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
        [provider("rust"), provider("python")],
    )
    .expect("progressive plan");
    assert_eq!(plan.routes[0].language_id, "python");
    assert_eq!(plan.routes[1].language_id, "rust");
    assert_eq!(
        plan.axes.clause_order,
        vec![
            SearchPlaybookClauseRef {
                axis: SearchPlaybookClauseAxis::Fd,
                block_index: 0,
            },
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
    assert_eq!(plan.axes.fd[0][0], "-t");
    assert_eq!(plan.work.filesystem_read_count, 0);
}

#[test]
fn request_without_a_producer_cannot_become_an_execution_plan() {
    let error = build_workspace_search_playbook_plan(
        &ProgressiveSearchPlaybookRequest {
            languages: None,
            documents: None,
            workspace: None,
            fd: vec![vec![".".to_owned()]],
            rg: vec![],
            tantivy: vec![],
            syntax: vec![],
            native_syntax: vec![],
            graph: vec![],
            clause_order: vec![SearchPlaybookClauseRef {
                axis: SearchPlaybookClauseAxis::Fd,
                block_index: 0,
            }],
        },
        WorkspaceSearchPlanBinding {
            project_id: "project".to_owned(),
            workspace_id: "workspace".to_owned(),
            content_generation_digest: "generation".to_owned(),
        },
        [provider("rust")],
    )
    .expect_err("execution planning requires a producer");
    assert!(error.contains("requires a producer"));
}
