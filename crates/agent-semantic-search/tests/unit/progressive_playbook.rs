// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use super::{
    ProgressiveSearchPlaybookError, ProgressiveSearchPlaybookRequest, SearchPlaybookClauseAxis,
    parse_progressive_search_playbook_args,
};

fn args(values: &[&str]) -> Vec<String> {
    values.iter().map(|value| (*value).to_owned()).collect()
}

#[test]
fn empty_invocation_is_an_incomplete_request() {
    let error = parse_progressive_search_playbook_args(&args(&["search", "playbook"]))
        .expect_err("empty Search has no alternate contract projection");
    assert!(matches!(
        error,
        ProgressiveSearchPlaybookError::IncompleteRequest(_)
    ));
    assert_eq!(error.reason_kind(), "search-playbook-request-incomplete");
}

#[test]
fn playbook_preserves_native_argv_and_pipe_expressions() {
    let request = parse_progressive_search_playbook_args(&args(&[
        "search",
        "playbook",
        "--language",
        "rust|python",
        "--documents",
        "org|md",
        "--rg",
        "-n",
        "-g",
        "*.rs",
        "--type",
        "rust",
        "transport|ClientFrame",
        "crates",
        "--tantivy",
        "title:\"surface failure\"^2 OR body:transport",
        "--syntax",
        "rust",
        "--treesitter-query",
        "((identifier) @symbol)",
        "--native-syntax",
        "rust://src/registry.rs#item/implementation/type/Registry",
        "--graph",
        "pgql",
        "MATCH (a)-[r]->(b) RETURN a, r, b",
    ]))
    .expect("complete playbook request");

    let ProgressiveSearchPlaybookRequest {
        language,
        documents,
        rg,
        clause_order,
        ..
    } = request;
    assert_eq!(language.as_deref(), Some("rust|python"));
    assert_eq!(documents.as_deref(), Some("org|md"));
    assert_eq!(
        rg[0],
        args(&[
            "-n",
            "-g",
            "*.rs",
            "--type",
            "rust",
            "transport|ClientFrame",
            "crates",
        ])
    );
    assert_eq!(
        clause_order
            .iter()
            .map(|clause| (clause.axis, clause.block_index))
            .collect::<Vec<_>>(),
        vec![
            (SearchPlaybookClauseAxis::Rg, 0),
            (SearchPlaybookClauseAxis::Tantivy, 0),
            (SearchPlaybookClauseAxis::Syntax, 0),
            (SearchPlaybookClauseAxis::NativeSyntax, 0),
            (SearchPlaybookClauseAxis::Graph, 0),
        ]
    );
}

#[test]
fn rg_requires_the_paired_tantivy_layout_input() {
    let error = parse_progressive_search_playbook_args(&args(&[
        "search",
        "playbook",
        "--language",
        "rust",
        "--rg",
        "-n",
        "RuntimeServingEndpoint|ClientFrame",
        ".",
    ]))
    .expect_err("default retrieval layout requires both native inputs");
    assert!(matches!(
        error,
        ProgressiveSearchPlaybookError::IncompleteRequest(_)
    ));
}

#[test]
fn target_bound_next_action_shape_is_complete() {
    let request = parse_progressive_search_playbook_args(&args(&[
        "search",
        "playbook",
        "--language",
        "typescript",
        "--rg",
        "-n",
        "-g",
        "*.ts",
        "-g",
        "*.tsx",
        "-F",
        "WorkflowExecution",
        ".",
        "--tantivy",
        "title:\"src/data/workflows.ts\" OR body:\"WorkflowExecution\"",
    ]))
    .expect("target-bound action carries both required native retrieval inputs");

    assert_eq!(request.language.as_deref(), Some("typescript"));
    assert_eq!(request.rg.len(), 1);
    assert_eq!(request.tantivy.len(), 1);
}

#[test]
fn repeated_clauses_preserve_agent_authored_priority() {
    let request = parse_progressive_search_playbook_args(&args(&[
        "search",
        "playbook",
        "--language",
        "rust",
        "--rg",
        "exact-owner",
        ".",
        "--syntax",
        "rust",
        "(identifier) @symbol",
        "--rg",
        "broad|fallback",
        ".",
        "--tantivy",
        "title:\"owner authority\"^2 OR body:fallback",
        "--graph",
        "gql",
        "MATCH (a)-[r]->(b) RETURN a, r, b",
    ]))
    .expect("ordered clauses");

    let ProgressiveSearchPlaybookRequest { clause_order, .. } = request;
    assert_eq!(
        clause_order
            .iter()
            .map(|clause| (clause.axis, clause.block_index))
            .collect::<Vec<_>>(),
        vec![
            (SearchPlaybookClauseAxis::Rg, 0),
            (SearchPlaybookClauseAxis::Syntax, 0),
            (SearchPlaybookClauseAxis::Rg, 1),
            (SearchPlaybookClauseAxis::Tantivy, 0),
            (SearchPlaybookClauseAxis::Graph, 0),
        ]
    );
}

#[test]
fn native_syntax_is_a_distinct_exact_selector_axis() {
    let request = parse_progressive_search_playbook_args(&args(&[
        "search",
        "playbook",
        "--language",
        "rust",
        "--rg",
        "-n",
        "Registry",
        "src",
        "--tantivy",
        "title:\"Registry\"^2 OR body:implementation",
        "--native-syntax",
        "rust://src/registry.rs#item/implementation/type/Registry",
    ]))
    .expect("canonical native-syntax selector");

    assert!(request.syntax.is_empty());
    assert_eq!(
        request.native_syntax,
        ["rust://src/registry.rs#item/implementation/type/Registry"]
    );
    assert_eq!(
        request
            .clause_order
            .iter()
            .map(|clause| clause.axis)
            .collect::<Vec<_>>(),
        [
            SearchPlaybookClauseAxis::Rg,
            SearchPlaybookClauseAxis::Tantivy,
            SearchPlaybookClauseAxis::NativeSyntax,
        ]
    );
}

#[test]
fn acquisition_after_graph_is_rejected() {
    let error = parse_progressive_search_playbook_args(&args(&[
        "search",
        "playbook",
        "--language",
        "rust",
        "--rg",
        "owner",
        ".",
        "--graph",
        "gql",
        "MATCH (a) RETURN a",
        "--syntax",
        "rust",
        "later",
    ]))
    .expect_err("graph is the final progressive filter");
    assert_eq!(
        error,
        ProgressiveSearchPlaybookError::InvalidClauseOrder(
            "retrieval and syntax inputs must precede the Graph barrier".to_owned()
        )
    );
}

#[test]
fn incomplete_request_is_rejected_without_a_contract_projection() {
    let error = parse_progressive_search_playbook_args(&args(&[
        "search",
        "playbook",
        "--language",
        "rust|python",
    ]))
    .expect_err("partial invocation must not return Example/Grammar");
    assert!(matches!(
        error,
        ProgressiveSearchPlaybookError::IncompleteRequest(_)
    ));
}

#[test]
fn invalid_typed_parameter_preserves_its_reason_kind() {
    let error = parse_progressive_search_playbook_args(&args(&[
        "search",
        "playbook",
        "--language",
        "rust",
        "--native-syntax",
        "src/lib.rs:10",
    ]))
    .expect_err("display locations are not exact selectors");
    assert_eq!(
        error.reason_kind(),
        "search-playbook-native-selector-invalid"
    );
    assert!(matches!(
        error,
        ProgressiveSearchPlaybookError::InvalidParameter { .. }
    ));
}

#[test]
fn receipt_identity_is_not_a_public_argument() {
    for option in [
        "--project-id",
        "--workspace-id",
        "--source-generation",
        "--round-id",
        "--context-digest",
    ] {
        let error = parse_progressive_search_playbook_args(&args(&[
            "search", "playbook", option, "forged",
        ]))
        .expect_err("receipt identity must not be Agent-authored");
        assert_eq!(
            error,
            ProgressiveSearchPlaybookError::UnsupportedOption(option.to_owned())
        );
    }
}
