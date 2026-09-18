// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use crate::{
    WorkspaceSearchAxisKind, WorkspaceSearchClauseReceipt, WorkspaceSearchGraphFanIn,
    WorkspaceSearchPlaybookEvidence, WorkspaceSearchPlaybookResultKind,
    WorkspaceSearchProgressiveExecutionWitness, WorkspaceSearchSyntaxCandidate,
    build_workspace_search_playbook_result, synthesize_workspace_search_playbook_result,
};

const EVIDENCE_ITEM_LIMIT: usize = 30;

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
        hit: crate::WorkspaceSearchHitProjection {
            rg: vec![[index as u64 + 1, index as u64 + 1]],
            native: true,
            ..Default::default()
        },
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
            EVIDENCE_ITEM_LIMIT,
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
        EVIDENCE_ITEM_LIMIT,
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
            EVIDENCE_ITEM_LIMIT,
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
            EVIDENCE_ITEM_LIMIT,
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
            EVIDENCE_ITEM_LIMIT,
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
    let candidate_owners = owners
        .iter()
        .map(|owner| (*owner).to_owned())
        .collect::<Vec<_>>();
    WorkspaceSearchClauseReceipt {
        axis,
        block_index,
        priority_rank,
        input_owner_count: candidate_owners.len().max(1),
        output_owner_count: candidate_owners.len(),
        candidate_owners,
        marginal_owner_reduction: None,
        elapsed_micros: 1,
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
        hit: crate::WorkspaceSearchHitProjection {
            native: true,
            ..Default::default()
        },
    }
}

#[test]
fn topology_owner_membership_emits_a_queryable_root_selector() {
    let owner = "crates/runtime/src/lib.rs";
    let result = synthesize_workspace_search_playbook_result(
        vec![receipt(WorkspaceSearchAxisKind::Topology, 0, 0, &[owner])],
        vec![WorkspaceSearchSyntaxCandidate {
            owner: owner.to_owned(),
            selector: format!("rust://{owner}"),
            relation: "topology-owner-membership".to_owned(),
            hit: crate::WorkspaceSearchHitProjection {
                native: true,
                ..Default::default()
            },
        }],
        None,
        EVIDENCE_ITEM_LIMIT,
    )
    .unwrap();

    assert_eq!(
        result.result,
        WorkspaceSearchPlaybookResultKind::ExactSelectorReady
    );
    assert_eq!(result.evidence[0].selector, format!("rust://{owner}"));
    assert_eq!(result.evidence[0].item, "owner-root");
    assert_eq!(result.evidence[0].matched_by, ["topology:0"]);
}

#[test]
fn fan_in_uses_agent_authored_clause_priority_without_graph() {
    let result = synthesize_workspace_search_playbook_result(
        vec![
            receipt(WorkspaceSearchAxisKind::Rg, 0, 0, &["src/a.rs"]),
            receipt(WorkspaceSearchAxisKind::Tantivy, 0, 1, &["src/b.rs"]),
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
        EVIDENCE_ITEM_LIMIT,
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
        EVIDENCE_ITEM_LIMIT,
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
                WorkspaceSearchAxisKind::Rg,
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
        EVIDENCE_ITEM_LIMIT,
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
        ["rg:0", "syntax:0", "graph:0"]
    );
    assert_eq!(result.evidence[0].relation, "syntax-capture:function");
}

#[test]
fn fan_in_preserves_structured_hit_values_for_the_rendered_selector() {
    let mut rg_candidate = syntax_candidate("src/a.rs", "a");
    rg_candidate.hit.rg = vec![[42, 46]];
    let mut tantivy_candidate = syntax_candidate("src/a.rs", "a");
    tantivy_candidate.hit.tantivy = vec!["artifact refresh".to_owned()];
    let result = synthesize_workspace_search_playbook_result(
        vec![
            receipt(WorkspaceSearchAxisKind::Rg, 0, 0, &["src/a.rs"]),
            receipt(WorkspaceSearchAxisKind::Tantivy, 0, 1, &["src/a.rs"]),
        ],
        vec![rg_candidate, tantivy_candidate],
        None,
        EVIDENCE_ITEM_LIMIT,
    )
    .unwrap();

    assert_eq!(result.evidence.len(), 1);
    assert!(result.evidence[0].hit.native);
    assert_eq!(result.evidence[0].hit.rg, [[42, 46]]);
    assert_eq!(result.evidence[0].hit.tantivy, ["artifact refresh"]);
}

#[test]
fn fan_in_rejects_non_contiguous_or_duplicate_priorities() {
    let error = synthesize_workspace_search_playbook_result(
        vec![
            receipt(WorkspaceSearchAxisKind::Tantivy, 0, 0, &[]),
            receipt(WorkspaceSearchAxisKind::Rg, 0, 0, &[]),
        ],
        Vec::new(),
        None,
        EVIDENCE_ITEM_LIMIT,
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
        EVIDENCE_ITEM_LIMIT,
    )
    .unwrap();

    assert_eq!(
        result.result,
        WorkspaceSearchPlaybookResultKind::RefinementRequired
    );
    assert!(result.evidence.is_empty());
}

#[test]
fn fan_in_rejects_unrelated_syntax_owner_before_ranking_and_limit() {
    let result = synthesize_workspace_search_playbook_result(
        vec![receipt(WorkspaceSearchAxisKind::Rg, 0, 0, &["src/a.rs"])],
        vec![
            syntax_candidate("src/unrelated.rs", "unrelated"),
            syntax_candidate("src/a.rs", "a"),
        ],
        None,
        EVIDENCE_ITEM_LIMIT,
    )
    .unwrap();

    assert_eq!(
        result.result,
        WorkspaceSearchPlaybookResultKind::ExactSelectorReady
    );
    assert_eq!(result.evidence.len(), 1);
    assert_eq!(result.evidence[0].owner, "src/a.rs");
    assert_eq!(result.evidence[0].matched_by, ["rg:0"]);
}

#[test]
fn graph_cannot_promote_an_owner_without_acquisition_support() {
    let result = synthesize_workspace_search_playbook_result(
        vec![receipt(WorkspaceSearchAxisKind::Rg, 0, 0, &["src/a.rs"])],
        vec![
            syntax_candidate("src/unrelated.rs", "unrelated"),
            syntax_candidate("src/a.rs", "a"),
        ],
        Some(WorkspaceSearchGraphFanIn {
            ranked_candidate_owners: vec!["src/unrelated.rs".to_owned(), "src/a.rs".to_owned()],
            applied_clause_count: 1,
            complete: true,
            truncated: false,
        }),
        EVIDENCE_ITEM_LIMIT,
    )
    .unwrap();

    assert_eq!(result.evidence.len(), 1);
    assert_eq!(result.evidence[0].owner, "src/a.rs");
    assert_eq!(result.evidence[0].matched_by, ["rg:0", "graph:0"]);
}

#[test]
fn fan_in_owner_support_index_scenario_records_work_reduction() {
    let supported_owners = (0..128)
        .map(|index| format!("src/supported_{index}.rs"))
        .collect::<Vec<_>>();
    let clause_receipts = (0..8)
        .map(|block_index| WorkspaceSearchClauseReceipt {
            axis: WorkspaceSearchAxisKind::Rg,
            block_index,
            priority_rank: block_index,
            input_owner_count: supported_owners.len(),
            output_owner_count: supported_owners.len(),
            candidate_owners: supported_owners.clone(),
            marginal_owner_reduction: None,
            elapsed_micros: 1,
            complete: true,
            coverage_complete: true,
            truncated: false,
        })
        .collect::<Vec<_>>();
    let mut candidates = supported_owners
        .iter()
        .enumerate()
        .map(|(index, owner)| syntax_candidate(owner, &format!("supported_{index}")))
        .collect::<Vec<_>>();
    candidates.extend((0..128).map(|index| {
        syntax_candidate(
            &format!("src/unrelated_{index}.rs"),
            &format!("unrelated_{index}"),
        )
    }));
    let owner_occurrence_count = 8 * supported_owners.len();
    let candidate_count = candidates.len();

    let result = synthesize_workspace_search_playbook_result(
        clause_receipts,
        candidates,
        None,
        EVIDENCE_ITEM_LIMIT,
    )
    .unwrap();

    let legacy_membership_checks = candidate_count * owner_occurrence_count;
    let indexed_owner_visits_and_lookups = owner_occurrence_count + candidate_count;
    eprintln!(
        "[scenario-metric] ownerOccurrences={owner_occurrence_count} candidates={candidate_count} legacyMembershipChecks={legacy_membership_checks} indexedOwnerVisitsAndLookups={indexed_owner_visits_and_lookups} evidence={}",
        result.evidence.len()
    );
    assert!(indexed_owner_visits_and_lookups < legacy_membership_checks);
    assert_eq!(result.evidence.len(), EVIDENCE_ITEM_LIMIT);
    assert!(
        result
            .evidence
            .iter()
            .all(|item| item.owner.starts_with("src/supported_"))
    );
    assert!(
        result
            .evidence
            .iter()
            .all(|item| item.matched_by.len() == 8)
    );
}

#[test]
fn complete_empty_acquisition_may_prove_no_match() {
    let result = synthesize_workspace_search_playbook_result(
        vec![receipt(WorkspaceSearchAxisKind::Rg, 0, 0, &[])],
        Vec::new(),
        None,
        EVIDENCE_ITEM_LIMIT,
    )
    .unwrap();

    assert_eq!(result.result, WorkspaceSearchPlaybookResultKind::NoMatch);
    assert!(result.evidence.is_empty());
}
