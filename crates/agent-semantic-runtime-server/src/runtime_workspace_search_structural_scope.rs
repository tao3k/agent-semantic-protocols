// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Maps retrieval evidence onto parser-owned structural selectors.

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

use super::{AspClientOperationError, RuntimeGrepMatch, WorkspaceSearchSyntaxCandidate};

pub(crate) fn resident_semantic_scope(
    generation: &crate::RuntimeQueryGeneration,
    owners: &BTreeSet<String>,
) -> Result<
    Option<
        std::sync::Arc<agent_semantic_client_db::runtime_resident_read::RuntimeResidentReadClient>,
    >,
    AspClientOperationError,
> {
    let resident = generation.resident();
    if !resident.has_semantic_owner_materialization_authority() {
        return Ok(None);
    }
    if !resident
        .semantic_owners_materialized(owners)
        .map_err(AspClientOperationError::Message)?
    {
        return Ok(None);
    }
    Ok(Some(generation.resident_arc()))
}

#[cfg(test)]
pub(crate) fn fused_file_context_scope(
    rg_scope: &BTreeSet<String>,
    tantivy_scope: &BTreeSet<String>,
) -> BTreeSet<String> {
    rg_scope.intersection(tantivy_scope).cloned().collect()
}

pub(crate) fn intersect_clause_owner_scopes(scopes: &[BTreeSet<String>]) -> BTreeSet<String> {
    let Some((smallest_index, smallest)) = scopes
        .iter()
        .enumerate()
        .min_by_key(|(_, scope)| scope.len())
    else {
        return BTreeSet::new();
    };
    let mut intersection = smallest.clone();
    for (index, scope) in scopes.iter().enumerate() {
        if index == smallest_index {
            continue;
        }
        intersection.retain(|owner| scope.contains(owner));
        if intersection.is_empty() {
            break;
        }
    }
    intersection
}

pub(crate) fn bounded_graph_seed_scope(
    scope: &BTreeSet<String>,
    limit: usize,
) -> (Vec<String>, bool) {
    (
        scope.iter().take(limit).cloned().collect(),
        scope.len() > limit,
    )
}

/// Select the only owners that may enter implicit parser grounding and
/// request-Topology construction after retrieval has completed.  Exact owner
/// membership remains in `complete_scope`; this cut is strictly downstream of
/// every Scheme set operation.
pub(crate) fn bounded_semantic_projection_scope<'a>(
    complete_scope: &BTreeSet<String>,
    grounding_matches: impl IntoIterator<Item = &'a RuntimeGrepMatch>,
    ranked_retrieval_branches: &[&[String]],
    limit: usize,
) -> (BTreeSet<String>, bool) {
    let mut selected = BTreeSet::new();
    if limit != 0 {
        // Values are `(branch support, grounding strength, fused rank)`. Keys
        // borrow the complete scope so ranking allocates no second owner set.
        let mut rank_signals = complete_scope
            .iter()
            .map(|owner| (owner.as_str(), (0usize, 0usize, 0usize)))
            .collect::<HashMap<_, _>>();
        for matched in grounding_matches {
            if let Some((_, grounding_strength, _)) =
                rank_signals.get_mut(matched.owner_path.as_str())
            {
                *grounding_strength += 1;
            }
        }
        // Reciprocal-rank fusion preserves the native rank of every complete
        // retrieval branch without introducing floating-point instability.
        // A branch contributes at most once per owner; exact Scheme set
        // membership was already computed in `complete_scope`.
        const RANK_SCALE: usize = 1_000_000;
        const RANK_OFFSET: usize = 60;
        for branch in ranked_retrieval_branches {
            let mut seen = HashSet::new();
            for (rank, owner) in branch.iter().enumerate() {
                if seen.insert(owner.as_str())
                    && let Some((branch_support, _, fused_rank)) =
                        rank_signals.get_mut(owner.as_str())
                {
                    *branch_support += 1;
                    *fused_rank += RANK_SCALE / RANK_OFFSET.saturating_add(rank).max(1);
                }
            }
        }
        let mut ranked = complete_scope.iter().collect::<Vec<_>>();
        let compare = |left_owner: &&String, right_owner: &&String| {
            let left = rank_signals
                .get(left_owner.as_str())
                .expect("complete owner has rank signals");
            let right = rank_signals
                .get(right_owner.as_str())
                .expect("complete owner has rank signals");
            right
                .0
                .cmp(&left.0)
                .then_with(|| right.1.cmp(&left.1))
                .then_with(|| right.2.cmp(&left.2))
                .then_with(|| left_owner.cmp(right_owner))
        };
        if ranked.len() > limit {
            ranked.select_nth_unstable_by(limit, compare);
            ranked.truncate(limit);
        }
        for owner in ranked {
            selected.insert(owner.clone());
        }
    }
    let truncated = selected.len() < complete_scope.len();
    (selected, truncated)
}

pub(crate) fn relation_neighbor_scope(
    resident: &agent_semantic_client_db::runtime_resident_read::RuntimeResidentReadClient,
    seed: &BTreeSet<String>,
    limit: usize,
) -> Result<BTreeSet<String>, AspClientOperationError> {
    let mut scope = seed.clone();
    if scope.len() >= limit {
        return Ok(scope);
    }
    let indexed = resident
        .indexed_owner_paths()
        .into_iter()
        .collect::<BTreeSet<_>>();
    for segment in resident
        .topology_source_segments_for_owner_scope(seed)
        .map_err(AspClientOperationError::Message)?
    {
        for relation in segment.relations {
            for endpoint in [&relation.relation.from, &relation.relation.to] {
                let owner = match endpoint.kind {
                    agent_semantic_content_identity::ProviderRelationEndpointKindV1::Owner => {
                        Some(endpoint.id.clone())
                    }
                    agent_semantic_content_identity::ProviderRelationEndpointKindV1::Item => {
                        agent_semantic_content_identity::CanonicalItemSelector::parse(
                            endpoint.id.clone(),
                        )
                        .ok()
                        .and_then(|selector| selector.owner_path().ok())
                    }
                };
                if let Some(owner) = owner
                    && indexed.contains(&owner)
                {
                    scope.insert(owner);
                    if scope.len() == limit {
                        return Ok(scope);
                    }
                }
            }
        }
    }
    Ok(scope)
}

pub(crate) fn syntax_candidates_enclosing_rg_matches<'a>(
    resident: &agent_semantic_client_db::runtime_resident_read::RuntimeResidentReadClient,
    owner_paths: &BTreeSet<String>,
    matches: impl IntoIterator<Item = &'a RuntimeGrepMatch>,
) -> Result<Vec<WorkspaceSearchSyntaxCandidate>, AspClientOperationError> {
    let matches = matches.into_iter().cloned().collect::<Vec<_>>();
    let owners = matches
        .iter()
        .map(|matched| matched.owner_path.clone())
        .filter(|owner| owner_paths.contains(owner))
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    if owners.is_empty() {
        return Ok(Vec::new());
    }
    let mut candidates = BTreeMap::<(String, String), WorkspaceSearchSyntaxCandidate>::new();
    for item in matches {
        if item.byte_start > item.byte_end {
            return Err(AspClientOperationError::Message(format!(
                "runtime-search-topology-grounding-unavailable: byte interval is reversed: owner={} start={} end={}",
                item.owner_path, item.byte_start, item.byte_end
            )));
        }
        let Some(anchor) = resident
            .smallest_enclosing_topology_anchor(&item.owner_path, item.byte_start, item.byte_end)
            .map_err(|error| {
                AspClientOperationError::Message(format!(
                    "runtime-search-topology-grounding-unavailable: {error}"
                ))
            })?
        else {
            // Top-level imports, attributes, and prose may satisfy the byte
            // predicate without identifying a parser-native item. They remain
            // exact retrieval evidence but cannot fabricate a selector.
            continue;
        };
        let candidate = candidates
            .entry((item.owner_path.clone(), anchor.structural_selector.clone()))
            .or_insert_with(|| WorkspaceSearchSyntaxCandidate {
                owner: item.owner_path.clone(),
                selector: anchor.structural_selector,
                relation: "topology-anchor-encloses:rg-match".to_owned(),
                hit: agent_semantic_search::WorkspaceSearchHitProjection {
                    native: true,
                    ..Default::default()
                },
            });
        candidate.hit.rg.push([item.owner_line, item.owner_line]);
    }
    for candidate in candidates.values_mut() {
        candidate.hit.rg.sort_unstable();
        candidate.hit.rg.dedup();
    }
    Ok(candidates.into_values().collect())
}

#[cfg(test)]
pub(crate) fn smallest_selector_overlapping_line(
    projection: &agent_semantic_search::NativeSyntaxProjection,
    line_start: usize,
    line_end: usize,
) -> Option<&agent_semantic_search::NativeSyntaxSelector> {
    projection
        .selectors
        .iter()
        .filter(|selector| selector.byte_start < line_end && line_start < selector.byte_end)
        .min_by(|left, right| {
            (left.byte_end - left.byte_start)
                .cmp(&(right.byte_end - right.byte_start))
                .then_with(|| left.selector.cmp(&right.selector))
        })
}

#[cfg(test)]
pub(crate) fn source_line_ranges(bytes: &[u8]) -> Vec<(usize, usize)> {
    let mut starts = vec![0];
    starts.extend(
        bytes
            .iter()
            .enumerate()
            .filter(|(_, byte)| **byte == b'\n')
            .map(|(index, _)| index + 1)
            .filter(|offset| *offset < bytes.len()),
    );
    starts
        .iter()
        .enumerate()
        .map(|(index, start)| {
            (
                *start,
                starts.get(index + 1).copied().unwrap_or(bytes.len()),
            )
        })
        .collect()
}

pub(crate) fn compile_graph_relation_pattern(
    block: &agent_semantic_search::GraphNativeBlock,
) -> Result<agent_semantic_search::ResidentGraphRelationPattern, AspClientOperationError> {
    agent_semantic_mrr::compile_graph_relation_pattern_v1(
        &block.language,
        &block.argv,
        Some("Owner"),
    )
    .map_err(|error| AspClientOperationError::Message(error.to_string()))
}
