// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later
//! Feature-qualified existing public Search route's Data aggregation boundary.
use super::RetrievalCompositionKind;
use crate::{RuntimeQueryGeneration, runtime_asp_client::AspClientOperationError};
use agent_semantic_mrr::{
    DataSearchCandidateComposition, DataSearchCompositionInput, DataSearchLeaf,
    compose_resident_data_search,
};
use agent_semantic_search::{WorkspaceSearchAxisKind, WorkspaceSearchPlaybookPlan};
use std::{
    collections::{BTreeMap, BTreeSet},
    num::NonZeroUsize,
};

type BranchScope = (WorkspaceSearchAxisKind, usize, BTreeSet<String>);
type BranchFlags = BTreeMap<(WorkspaceSearchAxisKind, usize), (usize, bool, bool)>;

pub(super) fn compose_retrieval_scope(
    plan: &WorkspaceSearchPlaybookPlan,
    generation: &RuntimeQueryGeneration,
    mode: RetrievalCompositionKind,
    scopes: &[BranchScope],
    flags: &BranchFlags,
) -> Result<BTreeSet<String>, AspClientOperationError> {
    let fail = AspClientOperationError::Message;
    let mode = match mode {
        RetrievalCompositionKind::Single => DataSearchCandidateComposition::Single,
        RetrievalCompositionKind::RankJoin => DataSearchCandidateComposition::RankJoin,
        RetrievalCompositionKind::Intersect => DataSearchCandidateComposition::Intersect,
        RetrievalCompositionKind::None => {
            return Err(fail("Data retrieval composition is absent".to_owned()));
        }
    };
    let mut leaves = scopes
        .iter()
        .map(|(axis, index, owners)| {
            let (priority, complete, truncated) = flags
                .get(&(*axis, *index))
                .copied()
                .ok_or_else(|| fail("Data retrieval branch has no source receipt".to_owned()))?;
            Ok((
                priority,
                DataSearchLeaf {
                    identity: format!("{}:{index}", axis.label()),
                    owners: owners.clone(),
                    complete,
                    truncated,
                },
            ))
        })
        .collect::<Result<Vec<_>, AspClientOperationError>>()?;
    leaves.sort_by_key(|(priority, _)| *priority);
    let max_observations = generation
        .resident()
        .indexed_owner_count()
        .checked_mul(leaves.len().saturating_add(1))
        .and_then(|count| NonZeroUsize::new(count.max(1)))
        .ok_or_else(|| fail("reasonKind=search-data-observation-budget-overflow".to_owned()))?;
    let query_bytes = serde_json::to_vec(&serde_json::json!({
        "composition": plan.axes.composition, "rg": plan.axes.rg,
        "tantivy": plan.axes.tantivy, "topology": plan.axes.topology,
    }))
    .map_err(|error| fail(error.to_string()))?;
    let query_digest = format!("blake3-256:{}", blake3::hash(&query_bytes));
    let view = generation.resident().resident_view_digest().map_err(fail)?;
    let abi = agent_semantic_search::search_playbook_composition_abi_digest();
    let output = compose_resident_data_search(DataSearchCompositionInput {
        project_id: &plan.project_id,
        workspace_id: &plan.workspace_id,
        runtime_generation: generation.generation_digest(),
        content_generation: generation.content_generation_digest(),
        expected_content_generation: &plan.content_generation_digest,
        resident_view_digest: &view,
        composition_abi: &abi,
        query_digest: &query_digest,
        mode,
        leaves: leaves.into_iter().map(|(_, leaf)| leaf).collect(),
        max_observations,
    })
    .map_err(fail)?;
    eprintln!(
        "[runtime-search-data-composition] receipt={}",
        output.receipt
    );
    Ok(output.owners)
}
