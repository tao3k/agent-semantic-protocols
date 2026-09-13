// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Save-token result projection for an executed Workspace Search Playbook.

use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

pub const WORKSPACE_SEARCH_PLAYBOOK_RESULT_SCHEMA_ID: &str =
    "agent.semantic-protocols.workspace-search-playbook-result";

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum WorkspaceSearchPlaybookResultKind {
    ExactSelectorReady,
    DisambiguationRequired,
    RelationshipSupported,
    NoMatch,
    Contradiction,
    RefinementRequired,
    ProviderContractFailure,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WorkspaceSearchPlaybookEvidence {
    pub owner: String,
    pub item: String,
    pub selector: String,
    pub matched_by: Vec<String>,
    pub relation: String,
    pub hit: WorkspaceSearchHitProjection,
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WorkspaceSearchHitProjection {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub rg: Vec<[u64; 2]>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tantivy: Vec<String>,
    #[serde(default, skip_serializing_if = "is_false")]
    pub native: bool,
}

impl WorkspaceSearchHitProjection {
    fn merge(&mut self, other: Self) {
        self.native |= other.native;
        self.rg.extend(other.rg);
        self.rg.sort_unstable();
        self.rg.dedup();
        self.tantivy.extend(other.tantivy);
        self.tantivy.sort();
        self.tantivy.dedup();
    }

    fn is_valid(&self) -> bool {
        (self.native || !self.rg.is_empty() || !self.tantivy.is_empty())
            && self
                .rg
                .iter()
                .all(|range| range[0] > 0 && range[0] <= range[1])
            && self.tantivy.iter().all(|query| !query.trim().is_empty())
            && self.tantivy.iter().collect::<BTreeSet<_>>().len() == self.tantivy.len()
    }
}

const fn is_false(value: &bool) -> bool {
    !*value
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum WorkspaceSearchAxisKind {
    Rg,
    Syntax,
    NativeSyntax,
    Tantivy,
}

impl WorkspaceSearchAxisKind {
    const fn label(self) -> &'static str {
        match self {
            Self::Rg => "rg",
            Self::Syntax => "syntax",
            Self::NativeSyntax => "native-syntax",
            Self::Tantivy => "tantivy",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkspaceSearchClauseReceipt {
    pub axis: WorkspaceSearchAxisKind,
    pub block_index: usize,
    pub priority_rank: usize,
    pub candidate_owners: Vec<String>,
    pub complete: bool,
    pub coverage_complete: bool,
    pub truncated: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkspaceSearchSyntaxCandidate {
    pub owner: String,
    pub selector: String,
    pub relation: String,
    pub hit: WorkspaceSearchHitProjection,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkspaceSearchGraphFanIn {
    pub ranked_candidate_owners: Vec<String>,
    pub applied_clause_count: usize,
    pub complete: bool,
    pub truncated: bool,
}

/// Runtime-only progressive completion authority. This witness is consumed
/// when the result is constructed and is never serialized to the Agent.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WorkspaceSearchProgressiveExecutionWitness {
    pub requested_clause_count: usize,
    pub completed_clause_count: usize,
    pub graph_requested: bool,
    pub graph_complete: bool,
}

impl WorkspaceSearchProgressiveExecutionWitness {
    #[must_use]
    pub const fn is_complete(self) -> bool {
        self.requested_clause_count > 0
            && self.requested_clause_count == self.completed_clause_count
            && (!self.graph_requested || self.graph_complete)
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WorkspaceSearchPlaybookResult {
    pub schema_id: String,
    pub schema_version: String,
    pub result: WorkspaceSearchPlaybookResultKind,
    pub evidence_item_limit: usize,
    pub evidence: Vec<WorkspaceSearchPlaybookEvidence>,
}

pub fn build_workspace_search_playbook_result(
    result: WorkspaceSearchPlaybookResultKind,
    evidence: Vec<WorkspaceSearchPlaybookEvidence>,
    evidence_item_limit: usize,
    witness: WorkspaceSearchProgressiveExecutionWitness,
) -> Result<WorkspaceSearchPlaybookResult, String> {
    if result != WorkspaceSearchPlaybookResultKind::ProviderContractFailure
        && !witness.is_complete()
    {
        return Err(
            "Search Playbook result requires every Agent-authored clause and requested Graph fan-in to complete"
                .to_owned(),
        );
    }
    if evidence_item_limit
        != agent_semantic_search_projection::WORKSPACE_SEARCH_PLAYBOOK_V1_EVIDENCE_ITEM_LIMIT
        || evidence.len() > evidence_item_limit
    {
        return Err(format!(
            "Search Playbook result exceeds its Top-{evidence_item_limit} evidence bound"
        ));
    }
    let mut selectors = BTreeSet::new();
    for item in &evidence {
        if item.owner.is_empty()
            || item.item.is_empty()
            || item.matched_by.is_empty()
            || item.relation.is_empty()
            || !item.hit.is_valid()
            || !is_exact_selector(&item.selector)
            || exact_selector_item(&item.selector) != Some(item.item.as_str())
            || item
                .matched_by
                .iter()
                .any(|clause| !is_clause_reference(clause))
            || item.matched_by.iter().collect::<BTreeSet<_>>().len() != item.matched_by.len()
            || !clause_references_are_progressive(&item.matched_by)
        {
            return Err(
                "Search Playbook evidence requires owner, exact item, exact selector, supporting clauses, and semantic relation"
                    .to_owned(),
            );
        }
        if !selectors.insert(item.selector.as_str()) {
            return Err("Search Playbook evidence contains a duplicate selector".to_owned());
        }
    }
    Ok(WorkspaceSearchPlaybookResult {
        schema_id: WORKSPACE_SEARCH_PLAYBOOK_RESULT_SCHEMA_ID.to_owned(),
        schema_version: "1".to_owned(),
        result,
        evidence_item_limit,
        evidence,
    })
}

pub fn synthesize_workspace_search_playbook_result(
    mut clause_receipts: Vec<WorkspaceSearchClauseReceipt>,
    syntax_candidates: Vec<WorkspaceSearchSyntaxCandidate>,
    graph_fan_in: Option<WorkspaceSearchGraphFanIn>,
    evidence_item_limit: usize,
) -> Result<WorkspaceSearchPlaybookResult, String> {
    if evidence_item_limit == 0 {
        return Err("Search Playbook evidence budget must be non-zero".to_owned());
    }
    let mut identities = BTreeSet::new();
    let mut priorities = BTreeSet::new();
    for receipt in &mut clause_receipts {
        let mut seen_owners = BTreeSet::new();
        receipt
            .candidate_owners
            .retain(|owner| seen_owners.insert(owner.clone()));
        if !identities.insert((receipt.axis, receipt.block_index))
            || !priorities.insert(receipt.priority_rank)
        {
            return Err("Search Playbook fan-in received duplicate clause identity".to_owned());
        }
    }
    if priorities.iter().copied().ne(0..clause_receipts.len()) {
        return Err(
            "Search Playbook clause priorities must be contiguous in Agent-authored order"
                .to_owned(),
        );
    }
    clause_receipts.sort_by_key(|receipt| receipt.priority_rank);
    let completed_clause_count = clause_receipts
        .iter()
        .filter(|receipt| receipt.complete)
        .count();
    let graph_requested = graph_fan_in.is_some();
    let graph_complete = graph_fan_in.as_ref().is_none_or(|graph| graph.complete);
    let witness = WorkspaceSearchProgressiveExecutionWitness {
        requested_clause_count: clause_receipts.len(),
        completed_clause_count,
        graph_requested,
        graph_complete,
    };
    if !witness.is_complete() {
        return Err("Search Playbook fan-in requires every requested stage to complete".to_owned());
    }
    let mut candidates_by_selector = BTreeMap::<String, WorkspaceSearchSyntaxCandidate>::new();
    for candidate in syntax_candidates {
        match candidates_by_selector.entry(candidate.selector.clone()) {
            std::collections::btree_map::Entry::Vacant(entry) => {
                entry.insert(candidate);
            }
            std::collections::btree_map::Entry::Occupied(mut entry) => {
                let existing = entry.get_mut();
                if existing.owner != candidate.owner {
                    return Err(
                        "Search Playbook selector candidates disagree on owner identity".to_owned(),
                    );
                }
                existing.hit.merge(candidate.hit);
            }
        }
    }
    let graph_positions = graph_fan_in.as_ref().map(|graph| {
        graph
            .ranked_candidate_owners
            .iter()
            .enumerate()
            .map(|(rank, owner)| (owner.as_str(), rank))
            .collect::<BTreeMap<_, _>>()
    });
    let mut ranked = candidates_by_selector
        .into_values()
        .filter_map(|candidate| {
            let graph_rank = match &graph_positions {
                Some(positions) => *positions.get(candidate.owner.as_str())?,
                None => usize::MAX,
            };
            let supporting = clause_receipts
                .iter()
                .filter(|receipt| receipt.candidate_owners.contains(&candidate.owner))
                .collect::<Vec<_>>();
            let first_priority = supporting
                .first()
                .map_or(usize::MAX, |receipt| receipt.priority_rank);
            let acquisition_rank = supporting
                .iter()
                .filter_map(|receipt| {
                    receipt
                        .candidate_owners
                        .iter()
                        .position(|owner| owner == &candidate.owner)
                })
                .min()
                .unwrap_or(usize::MAX);
            let supporting_labels = supporting
                .iter()
                .map(|receipt| format!("{}:{}", receipt.axis.label(), receipt.block_index))
                .collect::<Vec<_>>();
            let mut matched_by = supporting_labels;
            if let Some(graph) = &graph_fan_in {
                matched_by
                    .extend((0..graph.applied_clause_count).map(|index| format!("graph:{index}")));
            }
            Some((
                graph_rank,
                first_priority,
                acquisition_rank,
                supporting.len(),
                candidate,
                matched_by,
            ))
        })
        .collect::<Vec<_>>();
    ranked.sort_by(|left, right| {
        left.0
            .cmp(&right.0)
            .then_with(|| left.1.cmp(&right.1))
            .then_with(|| left.2.cmp(&right.2))
            .then_with(|| right.3.cmp(&left.3))
            .then_with(|| left.4.owner.cmp(&right.4.owner))
            .then_with(|| left.4.selector.cmp(&right.4.selector))
    });
    let evidence = ranked
        .iter()
        .take(evidence_item_limit)
        .map(
            |(_, _, _, _, candidate, matched_by)| WorkspaceSearchPlaybookEvidence {
                owner: candidate.owner.clone(),
                item: exact_selector_item(&candidate.selector)
                    .expect("syntax candidate carries an exact selector")
                    .to_owned(),
                selector: candidate.selector.clone(),
                matched_by: matched_by.clone(),
                relation: candidate.relation.clone(),
                hit: candidate.hit.clone(),
            },
        )
        .collect::<Vec<_>>();
    let relationship_supported = graph_requested && !evidence.is_empty();
    let acquisition_found_candidates = clause_receipts
        .iter()
        .any(|receipt| !receipt.candidate_owners.is_empty());
    let absence_proved = clause_receipts
        .iter()
        .all(|receipt| receipt.coverage_complete && !receipt.truncated);
    let absence_proved = absence_proved
        && !acquisition_found_candidates
        && graph_fan_in
            .as_ref()
            .is_none_or(|graph| graph.complete && !graph.truncated);
    let result = match (ranked.len(), relationship_supported, absence_proved) {
        (0, _, true) => WorkspaceSearchPlaybookResultKind::NoMatch,
        (0, _, false) => WorkspaceSearchPlaybookResultKind::RefinementRequired,
        (_, true, _) => WorkspaceSearchPlaybookResultKind::RelationshipSupported,
        (1, false, _) => WorkspaceSearchPlaybookResultKind::ExactSelectorReady,
        (_, false, _) => WorkspaceSearchPlaybookResultKind::DisambiguationRequired,
    };
    build_workspace_search_playbook_result(result, evidence, evidence_item_limit, witness)
}

fn is_exact_selector(selector: &str) -> bool {
    let Some((producer, identity)) = selector.split_once("://") else {
        return false;
    };
    !producer.is_empty()
        && producer
            .chars()
            .all(|value| value.is_ascii_lowercase() || value.is_ascii_digit() || value == '-')
        && identity
            .split_once("#item/")
            .is_some_and(|(owner, item)| !owner.is_empty() && !item.is_empty())
}

fn exact_selector_item(selector: &str) -> Option<&str> {
    selector.split_once("#item/").map(|(_, item)| item)
}

fn is_clause_reference(value: &str) -> bool {
    let Some((axis, index)) = value.split_once(':') else {
        return false;
    };
    matches!(
        axis,
        "rg" | "tantivy" | "syntax" | "native-syntax" | "graph"
    ) && !index.is_empty()
        && index.bytes().all(|byte| byte.is_ascii_digit())
}

fn clause_references_are_progressive(values: &[String]) -> bool {
    let mut graph_seen = false;
    for value in values {
        let graph = value.starts_with("graph:");
        if graph {
            graph_seen = true;
        } else if graph_seen {
            return false;
        }
    }
    true
}
