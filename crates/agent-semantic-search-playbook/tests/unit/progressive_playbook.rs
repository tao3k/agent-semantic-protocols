// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use agent_semantic_search_playbook::{
    ProgressiveSearchPlaybookError, SEARCH_PLAYBOOK_MAX_COMPOSITION_DEPTH,
    SearchPlaybookClauseAxis, SearchPlaybookComposition, SearchPlaybookNormalizedComposition,
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
      (rg "-n" "-g" "*.rs" "transport|ClientFrame")
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
          (rg "-n" "WorkflowExecution")
          (tantivy "title:\"WorkflowExecution\"^2 OR body:workflow")))
    "#;
    let request = parse_progressive_search_playbook_args(&command(source)).expect("workspace");
    assert_eq!(request.workspace.as_deref(), Some("workspace-main"));
}

#[test]
fn regex_ranked_text_and_structural_queries_do_not_require_a_partner() {
    let regex = parse_progressive_search_playbook_args(&command(
        r#"(search (producers (language rust)) (rg "RuntimeClient|ResolvedRoute"))"#,
    ))
    .expect("standalone regex route");
    assert_eq!(regex.rg.len(), 1);
    assert!(regex.tantivy.is_empty());
    assert!(regex.syntax.is_empty());

    let ranked = parse_progressive_search_playbook_args(&command(
        r#"(search (producers (language rust)) (tantivy "title:RuntimeClient^2 OR body:runtime"))"#,
    ))
    .expect("standalone ranked-text route");
    assert!(ranked.rg.is_empty());
    assert_eq!(ranked.tantivy.len(), 1);

    let structural = parse_progressive_search_playbook_args(&command(
        r#"(search (producers (documents org)) (syntax org "((headline) @item (#asp-eq? @item \"todo\" \"TODO\"))"))"#,
    ))
    .expect("standalone provider-native structural route");
    assert!(structural.rg.is_empty());
    assert!(structural.tantivy.is_empty());
    assert_eq!(structural.syntax.len(), 1);
}

#[test]
fn byte_identical_predicates_are_rejected_before_runtime_work() {
    let error = parse_progressive_search_playbook_args(&command(
        r#"(search (producers (language rust))
             (intersect (rg "RuntimeClient") (rg "RuntimeClient")))"#,
    ))
    .expect_err("an identical predicate cannot contribute evidence");
    assert_eq!(error.reason_kind(), "search-playbook-redundant-predicate");
}

#[test]
fn rg_cannot_introduce_a_second_workspace_scope() {
    let source = r#"
      (search
        (workspace "workspace-main")
        (producers (language rust))
        (intersect
          (rg "RuntimeClient|ResolvedRoute" "crates")
          (tantivy "title:RuntimeClient^2 OR body:runtime")))
    "#;
    let error = parse_progressive_search_playbook_args(&command(source))
        .expect_err("rg path must not override the top-level Workspace scope");
    assert_eq!(error.reason_kind(), "search-playbook-rg-scope-conflict");
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
          (intersect (rg "RuntimeClient|ResolvedRoute")
                     (tantivy "title:RuntimeClient^2 OR body:runtime"))
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
        (chain (intersect (rg "RuntimeClient|ResolvedRoute")
                          (tantivy "title:RuntimeClient^2 OR body:runtime"))
               (shell "rg" "RuntimeClient")))
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
          (chain (rg "RuntimeClient|ResolvedRoute")
                 (syntax rust "(identifier) @name"))
          (tantivy "title:RuntimeClient^2 OR body:runtime")))
    "#;
    let error = parse_progressive_search_playbook_args(&command(source))
        .expect_err("nested topology must not be flattened");
    assert_eq!(error.reason_kind(), "search-playbook-layout-invalid");
}

#[test]
fn recursive_composition_preserves_admitted_v1_execution_edges() {
    let source = r#"
      (search (producers (language rust))
        (chain
          (intersect
            (intersect
              (rg "RuntimeClient|ResolvedRoute")
              (rg "WorkspaceId"))
            (tantivy "title:RuntimeClient^2 OR body:runtime"))
          (chain
            (syntax rust "(identifier) @name")
            (graph gql "MATCH (a) RETURN a"))))
    "#;
    let request =
        parse_progressive_search_playbook_args(&command(source)).expect("recursive V1 Composition");
    assert!(matches!(
        request.composition,
        SearchPlaybookComposition::Chain(_)
    ));
    assert_eq!(request.composition.metrics().nodes, 9);
    assert_eq!(request.composition.metrics().depth, 4);
    let SearchPlaybookNormalizedComposition::Chain(children) = &request.normalized_composition
    else {
        panic!("normalized V1 wire tree must retain chain")
    };
    assert!(matches!(
        children.first(),
        Some(SearchPlaybookNormalizedComposition::Intersect(_))
    ));
    assert_eq!(request.rg.len(), 2);
    assert_eq!(
        request
            .clause_order
            .iter()
            .map(|entry| entry.axis)
            .collect::<Vec<_>>(),
        vec![
            SearchPlaybookClauseAxis::Rg,
            SearchPlaybookClauseAxis::Rg,
            SearchPlaybookClauseAxis::Tantivy,
            SearchPlaybookClauseAxis::Syntax,
            SearchPlaybookClauseAxis::Graph,
        ]
    );
}

#[test]
fn recursively_nested_chain_is_associative_before_v1_projection() {
    let source = r#"
      (search (producers (language rust))
        (chain
          (chain
            (intersect
              (rg "RuntimeClient|ResolvedRoute")
              (tantivy "title:RuntimeClient^2 OR body:runtime"))
            (syntax rust "(identifier) @name"))
          (graph gql "MATCH (a) RETURN a")))
    "#;
    let request = parse_progressive_search_playbook_args(&command(source))
        .expect("associative recursive chain");
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
            SearchPlaybookClauseAxis::Graph,
        ]
    );
}

#[test]
fn single_child_chain_preserves_the_existing_acquisition_layout() {
    let source = r#"
      (search (producers (language rust))
        (chain
          (intersect
            (rg "RuntimeClient|ResolvedRoute")
            (tantivy "title:RuntimeClient^2 OR body:runtime"))))
    "#;
    let request = parse_progressive_search_playbook_args(&command(source))
        .expect("one-child chain remains a valid recursive identity");
    assert!(matches!(
        request.composition,
        SearchPlaybookComposition::Chain(_)
    ));
    assert_eq!(request.rg.len(), 1);
    assert_eq!(request.tantivy.len(), 1);
    assert_eq!(request.clause_order.len(), 2);
}

#[test]
fn recursive_composition_depth_is_bounded_before_lowering() {
    let mut composition = concat!(
        "(intersect ",
        "(rg \"RuntimeClient|ResolvedRoute\") ",
        "(tantivy \"title:RuntimeClient^2 OR body:runtime\"))"
    )
    .to_owned();
    for _ in 0..SEARCH_PLAYBOOK_MAX_COMPOSITION_DEPTH {
        composition = format!("(intersect {composition} (rg \"WorkspaceId\"))");
    }
    let source = format!("(search (producers (language rust)) {composition})");
    let error = parse_progressive_search_playbook_args(&command(&source))
        .expect_err("over-depth Composition must fail before lowering");
    assert_eq!(
        error.reason_kind(),
        "search-playbook-composition-budget-exceeded"
    );
}

#[test]
fn recursive_composition_cannot_change_registered_domain_edges() {
    let source = r#"
      (search (producers (language rust))
        (chain
          (intersect
            (rg "RuntimeClient|ResolvedRoute")
            (tantivy "title:RuntimeClient^2 OR body:runtime"))
          (intersect
            (syntax rust "(identifier) @name")
            (syntax rust "(function_item) @item"))))
    "#;
    let error = parse_progressive_search_playbook_args(&command(source))
        .expect_err("structural intersect has no V1 layout equivalence");
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
