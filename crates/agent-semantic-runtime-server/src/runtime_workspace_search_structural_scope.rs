// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Maps retrieval evidence onto parser-owned structural selectors.

use std::collections::{BTreeMap, BTreeSet};

use super::{AspClientOperationError, RuntimeGrepMatch, WorkspaceSearchSyntaxCandidate};

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

pub(crate) fn syntax_candidates_enclosing_rg_matches<'a>(
    resident: &agent_semantic_client_db::runtime_resident_read::RuntimeResidentReadClient,
    owner_paths: &BTreeSet<String>,
    matches: impl IntoIterator<Item = &'a RuntimeGrepMatch>,
) -> Result<Vec<WorkspaceSearchSyntaxCandidate>, AspClientOperationError> {
    let matches = matches.into_iter().cloned().collect::<Vec<_>>();
    let owners = owner_paths.iter().cloned().collect::<Vec<_>>();
    if owners.is_empty() {
        return Ok(Vec::new());
    }
    let (projections, _, _) = resident
        .native_syntax_playbook_projection(&owners)
        .map_err(AspClientOperationError::Message)?;
    let projections = projections
        .into_iter()
        .map(|projection| (projection.owner_path.clone(), projection))
        .collect::<BTreeMap<_, _>>();
    let mut line_ranges = BTreeMap::new();
    for owner in &owners {
        let snapshot = resident
            .owner_snapshot(owner)
            .map_err(AspClientOperationError::Message)?
            .ok_or_else(|| {
                AspClientOperationError::Message(format!(
                    "rg syntax mapping owner disappeared: {owner}"
                ))
            })?;
        line_ranges.insert(owner.clone(), source_line_ranges(&snapshot.bytes));
    }

    let matched_owners = matches
        .iter()
        .map(|item| item.owner_path.clone())
        .collect::<BTreeSet<_>>();
    let mut candidates = BTreeMap::<(String, String), WorkspaceSearchSyntaxCandidate>::new();
    for owner in owner_paths.difference(&matched_owners) {
        let Some(projection) = projections.get(owner) else {
            continue;
        };
        for selector in &projection.selectors {
            candidates.insert(
                (owner.clone(), selector.selector.clone()),
                WorkspaceSearchSyntaxCandidate {
                    owner: owner.clone(),
                    selector: selector.selector.clone(),
                    relation: "native-parser:owner-scope".to_owned(),
                    hit: agent_semantic_search::WorkspaceSearchHitProjection {
                        native: true,
                        ..Default::default()
                    },
                },
            );
        }
    }
    for item in matches {
        let Some((line_start, line_end)) = line_ranges
            .get(&item.owner_path)
            .and_then(|ranges| {
                usize::try_from(item.owner_line)
                    .ok()?
                    .checked_sub(1)
                    .and_then(|line| ranges.get(line))
            })
            .copied()
        else {
            return Err(AspClientOperationError::Message(format!(
                "rg syntax mapping line is outside owner: owner={} line={}",
                item.owner_path, item.owner_line
            )));
        };
        let Some(projection) = projections.get(&item.owner_path) else {
            continue;
        };
        let enclosing = smallest_selector_overlapping_line(projection, line_start, line_end);
        if let Some(selector) = enclosing {
            let candidate = candidates
                .entry((item.owner_path.clone(), selector.selector.clone()))
                .or_insert_with(|| WorkspaceSearchSyntaxCandidate {
                    owner: item.owner_path.clone(),
                    selector: selector.selector.clone(),
                    relation: "syntax-encloses:rg-match".to_owned(),
                    hit: agent_semantic_search::WorkspaceSearchHitProjection {
                        native: true,
                        ..Default::default()
                    },
                });
            candidate.hit.rg.push([item.owner_line, item.owner_line]);
        }
    }
    for candidate in candidates.values_mut() {
        candidate.hit.rg.sort_unstable();
        candidate.hit.rg.dedup();
    }
    Ok(candidates.into_values().collect())
}

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
