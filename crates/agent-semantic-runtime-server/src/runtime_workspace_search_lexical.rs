// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Resident lexical set producers and their completeness receipts.

use std::collections::{BTreeMap, BTreeSet};

use agent_semantic_search::{
    WorkspaceSearchAxisKind, WorkspaceSearchClauseReceipt, WorkspaceSearchSyntaxCandidate,
};

use crate::RuntimeQueryGeneration;

use super::AspClientOperationError;
use super::workspace_search_playbook::intersect_clause_owner_scopes;

pub(super) struct SearchClauseMetrics {
    pub input_owner_count: usize,
    pub marginal_owner_reduction: Option<usize>,
    pub elapsed_micros: u64,
    pub coverage_complete: bool,
    pub truncated: bool,
}

pub(super) struct TantivyClauseResult {
    pub owners: Vec<String>,
    pub syntax_candidates: Vec<WorkspaceSearchSyntaxCandidate>,
    pub truncated: bool,
}

impl TantivyClauseResult {
    pub(super) fn require_complete_fused_scope(&self) -> Result<(), AspClientOperationError> {
        require_complete_intersection_branch(WorkspaceSearchAxisKind::Tantivy, self.truncated)
    }
}

pub(super) fn resident_grep_candidate_scope(
    candidate_plan: &agent_semantic_search::ResidentGrepCandidatePlan,
    tantivy_scope: &BTreeSet<String>,
    limit: usize,
    trigram_candidates: impl FnOnce() -> Result<
        (
            Vec<String>,
            agent_semantic_search::ResidentByteCoverageQueryReceipt,
        ),
        String,
    >,
) -> Result<
    (
        Vec<String>,
        agent_semantic_search::ResidentByteCoverageQueryReceipt,
    ),
    String,
> {
    if !candidate_plan.is_match_all() {
        let (mut owners, mut receipt) = trigram_candidates()?;
        owners.retain(|owner| tantivy_scope.contains(owner));
        receipt.candidate_count = owners.len();
        return Ok((owners, receipt));
    }
    if tantivy_scope.len() > limit {
        return Err(format!(
            "query-not-ready: resident GREP fused candidate budget exceeded: candidates={} limit={limit}",
            tantivy_scope.len()
        ));
    }
    Ok((
        tantivy_scope.iter().cloned().collect(),
        agent_semantic_search::ResidentByteCoverageQueryReceipt {
            requested_gram_count: 0,
            decoded_posting_count: 0,
            smallest_posting_count: 0,
            candidate_count: tantivy_scope.len(),
            lookup_nanos: 0,
        },
    ))
}

pub(super) fn require_complete_intersection_branch(
    axis: WorkspaceSearchAxisKind,
    truncated: bool,
) -> Result<(), AspClientOperationError> {
    if truncated {
        return Err(AspClientOperationError::Message(format!(
            "query-not-ready: {} candidate scope truncated before explicit intersection",
            axis.label()
        )));
    }
    Ok(())
}

pub(super) fn execute_tantivy_block(
    block: &[String],
    routes: &[agent_semantic_search::WorkspaceSearchPlaybookRoute],
    generation: &RuntimeQueryGeneration,
    exact_owner_scope: Option<&[String]>,
    limit: u32,
    selector_limit: usize,
) -> Result<TantivyClauseResult, AspClientOperationError> {
    let mut owners = Vec::new();
    let mut seen = BTreeSet::new();
    let mut truncated = false;
    let expression = block.join(" ");
    let analysis = agent_semantic_search::analyze_tantivy_query(&expression);
    if !analysis.is_admitted() {
        return Err(AspClientOperationError::Message(format!(
            "search-playbook-tantivy-expression-not-admitted: syntaxDiagnostics={:?} unsupportedFields={:?} missingFeatures={:?}",
            analysis.syntax_diagnostics, analysis.unsupported_fields, analysis.missing_features,
        )));
    }
    let generation_owner_limit = u32::try_from(generation.resident().indexed_owner_count())
        .unwrap_or(u32::MAX)
        .max(1);
    for route in routes {
        let language = agent_semantic_client_core::LanguageId::from(route.language_id.as_str());
        let result = generation
            .resident()
            .resident_tantivy_owner_paths(
                &expression,
                &language,
                exact_owner_scope,
                generation_owner_limit as usize,
            )
            .map_err(AspClientOperationError::Message)?;
        for owner_path in result {
            if generation.resident().contains_indexed_owner(&owner_path)
                && seen.insert(owner_path.clone())
            {
                if owners.len() == limit as usize {
                    truncated = true;
                    break;
                }
                owners.push(owner_path);
            }
        }
    }
    let owner_scope = owners.iter().cloned().collect::<BTreeSet<_>>();
    let admitted_languages = routes
        .iter()
        .map(|route| route.language_id.clone())
        .collect::<BTreeSet<_>>();
    let mut topology_hits = Vec::new();
    if analysis.selector_projection_complete {
        for selector_query in &analysis.selector_queries {
            topology_hits.extend(
                generation
                    .resident()
                    .read_topology_index(selector_query, selector_limit.saturating_add(1))
                    .map_err(AspClientOperationError::Message)?,
            );
        }
    }
    let syntax_candidates = agent_semantic_search::ranked_text_topology_selector_candidates(
        &expression,
        &owner_scope,
        &admitted_languages,
        topology_hits,
        selector_limit,
    )
    .map_err(AspClientOperationError::Message)?;
    Ok(TantivyClauseResult {
        owners,
        syntax_candidates,
        truncated,
    })
}

pub(super) fn complete_receipt(
    axis: WorkspaceSearchAxisKind,
    block_index: usize,
    priority_rank: usize,
    candidate_owners: Vec<String>,
    metrics: SearchClauseMetrics,
) -> WorkspaceSearchClauseReceipt {
    let output_owner_count = candidate_owners.len();
    WorkspaceSearchClauseReceipt {
        axis,
        block_index,
        priority_rank,
        candidate_owners,
        input_owner_count: metrics.input_owner_count,
        output_owner_count,
        marginal_owner_reduction: metrics.marginal_owner_reduction,
        elapsed_micros: metrics.elapsed_micros,
        complete: true,
        coverage_complete: metrics.coverage_complete,
        truncated: metrics.truncated,
    }
}

pub(super) fn elapsed_micros(started: std::time::Instant) -> u64 {
    started.elapsed().as_micros().min(u128::from(u64::MAX)) as u64
}

pub(super) fn branch_marginal_reductions(
    branches: &[(WorkspaceSearchAxisKind, usize, BTreeSet<String>)],
) -> BTreeMap<(WorkspaceSearchAxisKind, usize), usize> {
    if branches.len() < 2 {
        return BTreeMap::new();
    }
    let fused = intersect_clause_owner_scopes(
        &branches
            .iter()
            .map(|(_, _, scope)| scope.clone())
            .collect::<Vec<_>>(),
    );
    branches
        .iter()
        .enumerate()
        .map(|(excluded, (axis, block_index, _))| {
            let without_branch = intersect_clause_owner_scopes(
                &branches
                    .iter()
                    .enumerate()
                    .filter(|(index, _)| *index != excluded)
                    .map(|(_, (_, _, scope))| scope.clone())
                    .collect::<Vec<_>>(),
            );
            (
                (*axis, *block_index),
                without_branch.len().saturating_sub(fused.len()),
            )
        })
        .collect()
}
