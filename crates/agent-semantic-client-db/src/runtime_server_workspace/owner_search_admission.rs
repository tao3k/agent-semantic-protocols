// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-only

//! Shared exact owner-search admission for resident and durable generation adapters.

use super::{ExactProjectionKind, WorkspaceOwnerSearchSeedSnapshot};

pub(crate) fn selector_is_admitted(
    projection_kind: ExactProjectionKind,
    query_keys: &[String],
    query_terms: &[String],
) -> Result<bool, String> {
    if query_keys.iter().any(String::is_empty)
        || query_keys.windows(2).any(|pair| pair[0] >= pair[1])
    {
        return Err("workspace selector query keys are not canonical".to_owned());
    }
    Ok(projection_kind == ExactProjectionKind::Source
        && (query_terms.is_empty()
            || query_terms
                .iter()
                .any(|term| query_keys.binary_search(term).is_ok())))
}

pub(crate) fn finish_owner_search(
    mut seeds: Vec<WorkspaceOwnerSearchSeedSnapshot>,
    limit: usize,
) -> Result<(usize, Vec<WorkspaceOwnerSearchSeedSnapshot>), String> {
    seeds.sort_by(|left, right| left.selector.cmp(&right.selector));
    if seeds
        .windows(2)
        .any(|pair| pair[0].selector == pair[1].selector)
    {
        return Err("workspace generation contains duplicate source selectors".to_owned());
    }
    let candidate_count = seeds.len();
    seeds.truncate(limit);
    Ok((candidate_count, seeds))
}

#[cfg(test)]
#[path = "../../tests/unit/runtime_server_workspace/owner_search_admission.rs"]
mod tests;
