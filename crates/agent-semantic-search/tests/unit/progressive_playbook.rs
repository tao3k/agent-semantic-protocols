use super::{
    ProgressiveSearchPlaybookError, ProgressiveSearchPlaybookRequest, SearchPlaybookClauseAxis,
    parse_progressive_search_playbook_args,
};

fn args(values: &[&str]) -> Vec<String> {
    values.iter().map(|value| (*value).to_owned()).collect()
}

#[test]
fn empty_invocation_becomes_a_contract_index_query() {
    assert_eq!(
        parse_progressive_search_playbook_args(&args(&["search", "playbook"])),
        Ok(ProgressiveSearchPlaybookRequest::ContractQuery {
            languages: None,
            documents: None,
            workspace: None,
        })
    );
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
        "--graph",
        "pgql",
        "MATCH (a)-[r]->(b) RETURN a, r, b",
    ]))
    .expect("complete playbook request");

    let ProgressiveSearchPlaybookRequest::Execute {
        languages,
        fd,
        clause_order,
        ..
    } = request
    else {
        panic!("expected executable playbook request");
    };
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

    let ProgressiveSearchPlaybookRequest::Execute {
        rg, clause_order, ..
    } = request
    else {
        panic!("expected executable broad search");
    };
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

    let ProgressiveSearchPlaybookRequest::Execute { clause_order, .. } = request else {
        panic!("expected executable ordered search");
    };
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
fn incomplete_request_becomes_a_provider_contract_query() {
    let request = parse_progressive_search_playbook_args(&args(&[
        "search",
        "playbook",
        "--languages",
        "rust|python",
    ]))
    .expect("partial invocation is a contract query");
    assert_eq!(
        request,
        ProgressiveSearchPlaybookRequest::ContractQuery {
            languages: Some("rust|python".to_owned()),
            documents: None,
            workspace: None,
        }
    );
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
