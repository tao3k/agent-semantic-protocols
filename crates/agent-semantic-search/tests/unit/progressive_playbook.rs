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
fn intent_is_not_a_playbook_parameter() {
    let error = parse_progressive_search_playbook_args(&args(&[
        "search", "playbook", "--intent", "diagnose",
    ]))
    .expect_err("reasoning intent belongs to the Agent context");
    assert_eq!(
        error,
        ProgressiveSearchPlaybookError::UnsupportedOption("--intent".to_owned())
    );
}

#[test]
fn playbook_preserves_native_argv_and_pipe_expressions() {
    let request = parse_progressive_search_playbook_args(&args(&[
        "search",
        "playbook",
        "--languages",
        "rust|python",
        "--documents",
        "org|md",
        "--fd",
        "-t",
        "f",
        "runtime|client",
        ".",
        "--rg",
        "-n",
        "transport|ClientFrame",
        ".",
        "--tantivy",
        "surface failure|transport ownership",
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
        languages,
        fd,
        clause_order,
        ..
    } = request;
    assert_eq!(languages.as_deref(), Some("rust|python"));
    assert_eq!(fd[0], args(&["-t", "f", "runtime|client", "."]));
    assert_eq!(
        clause_order
            .iter()
            .map(|clause| (clause.axis, clause.block_index))
            .collect::<Vec<_>>(),
        vec![
            (SearchPlaybookClauseAxis::Fd, 0),
            (SearchPlaybookClauseAxis::Rg, 0),
            (SearchPlaybookClauseAxis::Tantivy, 0),
            (SearchPlaybookClauseAxis::Syntax, 0),
            (SearchPlaybookClauseAxis::NativeSyntax, 0),
            (SearchPlaybookClauseAxis::Graph, 0),
        ]
    );
}

#[test]
fn one_acquisition_clause_executes_without_graph() {
    let request = parse_progressive_search_playbook_args(&args(&[
        "search",
        "playbook",
        "--languages",
        "rust",
        "--rg",
        "-n",
        "RuntimeServingEndpoint|ClientFrame",
        ".",
    ]))
    .expect("first broad search does not require graph reasoning");

    let ProgressiveSearchPlaybookRequest {
        rg, clause_order, ..
    } = request;
    assert_eq!(rg.len(), 1);
    assert_eq!(clause_order.len(), 1);
    assert_eq!(clause_order[0].axis, SearchPlaybookClauseAxis::Rg);
    assert_eq!(clause_order[0].block_index, 0);
}

#[test]
fn repeated_clauses_preserve_agent_authored_priority() {
    let request = parse_progressive_search_playbook_args(&args(&[
        "search",
        "playbook",
        "--languages",
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
            (SearchPlaybookClauseAxis::Graph, 0),
        ]
    );
}

#[test]
fn native_syntax_is_a_distinct_exact_selector_axis() {
    let request = parse_progressive_search_playbook_args(&args(&[
        "search",
        "playbook",
        "--languages",
        "rust",
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
        request.clause_order[0].axis,
        SearchPlaybookClauseAxis::NativeSyntax
    );
}

#[test]
fn acquisition_after_graph_is_rejected() {
    let error = parse_progressive_search_playbook_args(&args(&[
        "search",
        "playbook",
        "--languages",
        "rust",
        "--rg",
        "owner",
        ".",
        "--graph",
        "gql",
        "MATCH (a) RETURN a",
        "--fd",
        "later",
        ".",
    ]))
    .expect_err("graph is the final progressive filter");
    assert_eq!(
        error,
        ProgressiveSearchPlaybookError::InvalidClauseOrder(
            "Search Playbook acquisition clauses must precede --graph".to_owned()
        )
    );
}

#[test]
fn incomplete_request_is_rejected_without_a_contract_projection() {
    let error = parse_progressive_search_playbook_args(&args(&[
        "search",
        "playbook",
        "--languages",
        "rust|python",
    ]))
    .expect_err("partial invocation must not return Example/Grammar");
    assert!(matches!(
        error,
        ProgressiveSearchPlaybookError::IncompleteRequest(_)
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
