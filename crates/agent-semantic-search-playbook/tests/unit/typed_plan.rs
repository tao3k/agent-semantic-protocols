// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Pure-typed Search Playbook IR tests (no pseudo S-expression string parser).

use agent_semantic_search_playbook::{
    GraphNativeBlock, ProducerNativeBlock, SearchPlaybookClauseAxis, SearchPlaybookComposition,
    SearchPlaybookLeaf, SearchPlaybookPlanInput, TopologyOwnerMembershipBlock,
    admit_search_playbook_plan,
};

#[test]
fn typed_search_plan_admits_chained_intersect_syntax_and_graph_without_string_parsing() {
    let plan = SearchPlaybookPlanInput {
        workspace: Some("marlin-core".to_owned()),
        languages: vec!["rust".to_owned(), "scheme".to_owned()],
        documents: vec!["org".to_owned()],
        composition: SearchPlaybookComposition::Chain(vec![
            SearchPlaybookComposition::Intersect(vec![
                SearchPlaybookComposition::Leaf(SearchPlaybookLeaf::Rg(vec![
                    "-n".to_owned(),
                    "-e".to_owned(),
                    "publish|refresh".to_owned(),
                ])),
                SearchPlaybookComposition::Leaf(SearchPlaybookLeaf::Tantivy(vec![
                    "(title:\"search engine\"^2 OR body:\"generation\") AND marlin".to_owned(),
                ])),
                SearchPlaybookComposition::Leaf(SearchPlaybookLeaf::Topology(
                    TopologyOwnerMembershipBlock {
                        kind: "file".to_owned(),
                        exact_path: None,
                        path_prefix: Some("crates/".to_owned()),
                        extension: Some("rs".to_owned()),
                        path_glob: None,
                    },
                )),
            ]),
            SearchPlaybookComposition::Leaf(SearchPlaybookLeaf::Syntax(ProducerNativeBlock {
                producer: "rust".to_owned(),
                argv: vec!["(function_item name: (identifier) @name)".to_owned()],
            })),
            SearchPlaybookComposition::Leaf(SearchPlaybookLeaf::Graph(GraphNativeBlock {
                language: "gql".to_owned(),
                argv: vec!["MATCH (a)-[r:CALLS]->(b) RETURN a, b".to_owned()],
            })),
        ]),
    };

    let request = admit_search_playbook_plan(plan).expect("valid typed search plan");
    assert_eq!(request.workspace.as_deref(), Some("marlin-core"));
    assert_eq!(request.language.as_deref(), Some("rust|scheme"));
    assert_eq!(request.documents.as_deref(), Some("org"));
    assert_eq!(request.rg.len(), 1);
    assert_eq!(request.tantivy.len(), 1);
    assert_eq!(request.topology.len(), 1);
    assert_eq!(request.syntax.len(), 1);
    assert_eq!(request.graph.len(), 1);
    assert_eq!(request.clause_order.len(), 5);
    assert_eq!(request.clause_order[0].axis, SearchPlaybookClauseAxis::Rg);
    assert_eq!(
        request.clause_order[4].axis,
        SearchPlaybookClauseAxis::Graph
    );
}

#[test]
fn typed_search_plan_rejects_rg_path_traversal_and_graph_first_layout() {
    let rg_with_path = SearchPlaybookPlanInput {
        workspace: Some("marlin-core".to_owned()),
        languages: vec!["rust".to_owned()],
        documents: vec![],
        composition: SearchPlaybookComposition::Leaf(SearchPlaybookLeaf::Rg(vec![
            "-e".to_owned(),
            "foo".to_owned(),
            "crates/".to_owned(),
        ])),
    };
    let err = admit_search_playbook_plan(rg_with_path).expect_err("rg path must fail");
    assert_eq!(err.reason_kind(), "search-playbook-rg-scope-conflict");

    let graph_only = SearchPlaybookPlanInput {
        workspace: None,
        languages: vec!["rust".to_owned()],
        documents: vec![],
        composition: SearchPlaybookComposition::Leaf(SearchPlaybookLeaf::Graph(GraphNativeBlock {
            language: "gql".to_owned(),
            argv: vec!["MATCH (n) RETURN n".to_owned()],
        })),
    };
    let err = admit_search_playbook_plan(graph_only).expect_err("graph-first must fail");
    assert_eq!(err.reason_kind(), "search-playbook-layout-invalid");
}
