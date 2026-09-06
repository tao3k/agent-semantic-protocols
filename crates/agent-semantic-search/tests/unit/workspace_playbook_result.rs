use crate::{
    WorkspaceSearchAxisKind, WorkspaceSearchClauseReceipt, WorkspaceSearchGraphFanIn,
    WorkspaceSearchPlaybookEvidence, WorkspaceSearchPlaybookResultKind,
    WorkspaceSearchProgressiveExecutionWitness, WorkspaceSearchSyntaxCandidate,
    build_workspace_search_playbook_result, synthesize_workspace_search_playbook_result,
};

fn complete_witness() -> WorkspaceSearchProgressiveExecutionWitness {
    WorkspaceSearchProgressiveExecutionWitness {
        requested_clause_count: 2,
        completed_clause_count: 2,
        graph_requested: true,
        graph_complete: true,
    }
}

fn evidence(index: usize) -> WorkspaceSearchPlaybookEvidence {
    WorkspaceSearchPlaybookEvidence {
        owner: format!("src/item_{index}.rs"),
        item: "function/run".to_owned(),
        selector: format!("rust://src/item_{index}.rs#item/function/run"),
        matched_by: vec!["rg:0".to_owned(), "graph:0".to_owned()],
        relation: "syntax-capture:function".to_owned(),
    }
}

#[test]
fn result_requires_every_clause_and_requested_graph_fan_in() {
    let mut witness = complete_witness();
    witness.graph_complete = false;
    assert!(
        build_workspace_search_playbook_result(
            WorkspaceSearchPlaybookResultKind::RefinementRequired,
            Vec::new(),
            witness,
        )
        .unwrap_err()
        .contains("Graph fan-in")
    );
}

#[test]
fn result_is_selector_only_and_top_thirty() {
    let result = build_workspace_search_playbook_result(
        WorkspaceSearchPlaybookResultKind::ExactSelectorReady,
        vec![evidence(2), evidence(1)],
        complete_witness(),
    )
    .unwrap();
    let value = serde_json::to_value(result).unwrap();
    assert_eq!(value["evidence"].as_array().unwrap().len(), 2);
    assert!(value.get("plan").is_none());
    assert!(value.get("next").is_none());
    assert!(value.get("digest").is_none());
    assert!(value["evidence"][0].get("source").is_none());

    assert!(
        build_workspace_search_playbook_result(
            WorkspaceSearchPlaybookResultKind::ExactSelectorReady,
            (0..31).map(evidence).collect(),
            complete_witness(),
        )
        .unwrap_err()
        .contains("Top-30")
    );
}

#[test]
fn result_rejects_non_exact_selector() {
    let mut invalid = evidence(0);
    invalid.selector = "src/item_0.rs:12".to_owned();
    assert!(
        build_workspace_search_playbook_result(
            WorkspaceSearchPlaybookResultKind::ExactSelectorReady,
            vec![invalid],
            complete_witness(),
        )
        .is_err()
    );
}

#[test]
fn result_rejects_graph_provenance_before_acquisition_provenance() {
    let mut invalid = evidence(0);
    invalid.matched_by = vec!["graph:0".to_owned(), "rg:0".to_owned()];
    assert!(
        build_workspace_search_playbook_result(
            WorkspaceSearchPlaybookResultKind::ExactSelectorReady,
            vec![invalid],
            complete_witness(),
        )
        .is_err()
    );
}

fn receipt(
    axis: WorkspaceSearchAxisKind,
    block_index: usize,
    priority_rank: usize,
    owners: &[&str],
) -> WorkspaceSearchClauseReceipt {
    WorkspaceSearchClauseReceipt {
        axis,
        block_index,
        priority_rank,
        candidate_owners: owners.iter().map(|owner| (*owner).to_owned()).collect(),
        complete: true,
        coverage_complete: true,
        truncated: false,
    }
}

fn syntax_candidate(owner: &str, item: &str) -> WorkspaceSearchSyntaxCandidate {
    WorkspaceSearchSyntaxCandidate {
        owner: owner.to_owned(),
        selector: format!("rust://{owner}#item/function/{item}"),
        relation: "syntax-capture:function".to_owned(),
    }
}

#[test]
fn fan_in_uses_agent_authored_clause_priority_without_graph() {
    let result = synthesize_workspace_search_playbook_result(
        vec![
            receipt(WorkspaceSearchAxisKind::Rg, 0, 0, &["src/a.rs"]),
            receipt(WorkspaceSearchAxisKind::Fd, 0, 1, &["src/b.rs"]),
            receipt(
                WorkspaceSearchAxisKind::Syntax,
                0,
                2,
                &["src/a.rs", "src/b.rs"],
            ),
        ],
        vec![
            syntax_candidate("src/b.rs", "b"),
            syntax_candidate("src/a.rs", "a"),
        ],
        None,
    )
    .unwrap();
    assert_eq!(
        result.result,
        WorkspaceSearchPlaybookResultKind::DisambiguationRequired
    );
    assert_eq!(result.evidence[0].owner, "src/a.rs");
    assert_eq!(result.evidence[0].matched_by, ["rg:0", "syntax:0"]);
    assert_eq!(result.evidence[0].relation, "syntax-capture:function");
}

#[test]
fn fan_in_preserves_native_candidate_rank_before_lexical_tie_breaking() {
    let result = synthesize_workspace_search_playbook_result(
        vec![receipt(
            WorkspaceSearchAxisKind::Rg,
            0,
            0,
            &["src/high_rank.rs", "src/alphabetically_first.rs"],
        )],
        vec![
            syntax_candidate("src/alphabetically_first.rs", "lower_rank"),
            syntax_candidate("src/high_rank.rs", "higher_rank"),
        ],
        None,
    )
    .unwrap();

    assert_eq!(result.evidence[0].owner, "src/high_rank.rs");
    assert_eq!(result.evidence[1].owner, "src/alphabetically_first.rs");
}

#[test]
fn graph_is_a_filtering_and_ranking_fan_in_not_an_independent_vote() {
    let result = synthesize_workspace_search_playbook_result(
        vec![
            receipt(
                WorkspaceSearchAxisKind::Fd,
                0,
                0,
                &["src/a.rs", "src/b.rs", "src/c.rs"],
            ),
            receipt(
                WorkspaceSearchAxisKind::Syntax,
                0,
                1,
                &["src/a.rs", "src/b.rs", "src/c.rs"],
            ),
        ],
        vec![
            syntax_candidate("src/a.rs", "a"),
            syntax_candidate("src/b.rs", "b"),
            syntax_candidate("src/c.rs", "c"),
        ],
        Some(WorkspaceSearchGraphFanIn {
            ranked_candidate_owners: vec!["src/b.rs".to_owned(), "src/a.rs".to_owned()],
            applied_clause_count: 1,
            complete: true,
            truncated: false,
        }),
    )
    .unwrap();
    assert_eq!(
        result.result,
        WorkspaceSearchPlaybookResultKind::RelationshipSupported
    );
    assert_eq!(result.evidence[0].owner, "src/b.rs");
    assert_eq!(result.evidence[1].owner, "src/a.rs");
    assert!(result.evidence.iter().all(|item| item.owner != "src/c.rs"));
    assert_eq!(
        result.evidence[0].matched_by,
        ["fd:0", "syntax:0", "graph:0"]
    );
    assert_eq!(result.evidence[0].relation, "syntax-capture:function");
}

#[test]
fn fan_in_rejects_non_contiguous_or_duplicate_priorities() {
    let error = synthesize_workspace_search_playbook_result(
        vec![
            receipt(WorkspaceSearchAxisKind::Fd, 0, 0, &[]),
            receipt(WorkspaceSearchAxisKind::Rg, 0, 0, &[]),
        ],
        Vec::new(),
        None,
    )
    .unwrap_err();
    assert!(error.contains("duplicate clause identity"));
}

#[test]
fn acquisition_candidates_without_exact_syntax_mapping_require_refinement() {
    let result = synthesize_workspace_search_playbook_result(
        vec![receipt(WorkspaceSearchAxisKind::Rg, 0, 0, &["src/lib.rs"])],
        Vec::new(),
        None,
    )
    .unwrap();

    assert_eq!(
        result.result,
        WorkspaceSearchPlaybookResultKind::RefinementRequired
    );
    assert!(result.evidence.is_empty());
    assert!(result.query_grammar.is_none());
}

#[test]
fn complete_empty_acquisition_may_prove_no_match() {
    let result = synthesize_workspace_search_playbook_result(
        vec![receipt(WorkspaceSearchAxisKind::Rg, 0, 0, &[])],
        Vec::new(),
        None,
    )
    .unwrap();

    assert_eq!(result.result, WorkspaceSearchPlaybookResultKind::NoMatch);
    assert!(result.evidence.is_empty());
    assert!(result.query_grammar.is_none());
}
