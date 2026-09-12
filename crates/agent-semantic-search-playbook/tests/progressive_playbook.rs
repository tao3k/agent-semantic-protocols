// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use agent_semantic_search_playbook::{
    ProgressiveSearchPlaybookError, SearchPlaybookClauseAxis,
    parse_progressive_search_playbook_args, parse_search_playbook_producer_declaration,
};

fn command(source: &str) -> Vec<String> {
    vec![
        "search".to_owned(),
        "playbook".to_owned(),
        source.to_owned(),
    ]
}

const SOURCE: &str = r#"
(search
  (producers (language rust python) (documents org))
  (chain
    (intersect
      (rg "-n" "-g" "*.rs" "transport|ClientFrame" "crates")
      (tantivy "title:\"surface failure\"^2 OR body:transport"))
    (syntax rust "((identifier) @symbol)")
    (native-syntax "rust://src/registry.rs#item/implementation/type/Registry")
    (graph pgql "MATCH (a)-[r]->(b) RETURN a, r, b")))
"#;

#[test]
fn empty_invocation_is_incomplete() {
    let error =
        parse_progressive_search_playbook_args(&["search".to_owned(), "playbook".to_owned()])
            .expect_err("source is required");
    assert_eq!(error.reason_kind(), "search-playbook-request-incomplete");
}

#[test]
fn one_scheme_expression_lowers_to_the_existing_request() {
    let request = parse_progressive_search_playbook_args(&command(SOURCE))
        .expect("complete Scheme Search Playbook");
    assert_eq!(request.language.as_deref(), Some("rust|python"));
    assert_eq!(request.documents.as_deref(), Some("org"));
    assert_eq!(request.rg[0][0], "-n");
    assert_eq!(request.tantivy.len(), 1);
    assert_eq!(request.syntax[0].producer, "rust");
    assert_eq!(request.graph[0].language, "pgql");
    assert_eq!(
        request
            .clause_order
            .iter()
            .map(|entry| entry.axis)
            .collect::<Vec<_>>(),
        vec![
            SearchPlaybookClauseAxis::Rg,
            SearchPlaybookClauseAxis::Tantivy,
            SearchPlaybookClauseAxis::Syntax,
            SearchPlaybookClauseAxis::NativeSyntax,
            SearchPlaybookClauseAxis::Graph,
        ]
    );
}

#[test]
fn registered_workspace_is_inside_the_expression() {
    let source = r#"
      (search
        (workspace "workspace-main")
        (producers (language typescript))
        (intersect
          (rg "-n" "WorkflowExecution" ".")
          (tantivy "title:\"WorkflowExecution\"^2 OR body:workflow")))
    "#;
    let request = parse_progressive_search_playbook_args(&command(source)).expect("workspace");
    assert_eq!(request.workspace.as_deref(), Some("workspace-main"));
}

#[test]
fn old_flag_surface_is_not_a_compatibility_path() {
    let error = parse_progressive_search_playbook_args(&[
        "search".to_owned(),
        "playbook".to_owned(),
        "--language".to_owned(),
        "rust".to_owned(),
    ])
    .expect_err("only one Scheme expression is accepted");
    assert_eq!(error.reason_kind(), "search-playbook-source-arity-invalid");
}

#[test]
fn producer_declaration_is_available_before_leaf_admission() {
    let declaration = parse_search_playbook_producer_declaration(
        r#"(search (producers (language rust python) (documents org)) (rg "--glob"))"#,
    )
    .expect("producer declaration does not evaluate native leaves");
    assert_eq!(declaration.language, ["rust", "python"]);
    assert_eq!(declaration.documents, ["org"]);
}

#[test]
fn graph_remains_the_final_barrier() {
    let source = r#"
      (search (producers (language rust))
        (chain
          (intersect (rg "owner" ".")
                     (tantivy "title:owner^2 OR body:authority"))
          (graph gql "MATCH (a) RETURN a")
          (syntax rust "(identifier) @name")))
    "#;
    let error = parse_progressive_search_playbook_args(&command(source))
        .expect_err("syntax after Graph is invalid");
    assert!(matches!(
        error,
        ProgressiveSearchPlaybookError::InvalidClauseOrder(_)
    ));
}

#[test]
fn unknown_composition_operator_fails_closed() {
    let source = r#"
      (search (producers (language rust))
        (chain (intersect (rg "owner" ".")
                          (tantivy "title:owner^2 OR body:authority"))
               (shell "rg" "owner")))
    "#;
    let error =
        parse_progressive_search_playbook_args(&command(source)).expect_err("unknown operator");
    assert_eq!(error.reason_kind(), "search-playbook-operator-unsupported");
}

#[test]
fn v1_rejects_topology_that_the_normalized_request_cannot_preserve() {
    let source = r#"
      (search (producers (language rust))
        (intersect
          (chain (rg "owner" ".")
                 (syntax rust "(identifier) @name"))
          (tantivy "title:owner^2 OR body:authority")))
    "#;
    let error = parse_progressive_search_playbook_args(&command(source))
        .expect_err("nested topology must not be flattened");
    assert_eq!(error.reason_kind(), "search-playbook-layout-invalid");
}

#[test]
fn tree_sitter_reports_incomplete_source_location() {
    let error =
        parse_progressive_search_playbook_args(&command("(search (producers (language rust))"))
            .expect_err("unclosed Scheme source");
    assert!(matches!(
        error.reason_kind(),
        "scheme-source-invalid-syntax" | "scheme-source-missing-syntax"
    ));
}
