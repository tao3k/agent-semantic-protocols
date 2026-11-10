// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GraphCandidateSparsityInput {
    pub path: String,
    pub symbol: String,
}

impl GraphCandidateSparsityInput {
    pub fn new(path: impl Into<String>, symbol: impl Into<String>) -> Self {
        Self {
            path: path.into(),
            symbol: symbol.into(),
        }
    }
}

pub fn select_sparse_graph_candidate_indices(
    candidates: &[GraphCandidateSparsityInput],
    limit: usize,
) -> Vec<usize> {
    let groups = candidate_indices_by_symbol(candidates);
    let max_depth = groups.iter().map(Vec::len).max().unwrap_or(0);
    (0..max_depth)
        .flat_map(|depth| {
            groups
                .iter()
                .filter_map(move |group| group.get(depth).copied())
        })
        .take(limit)
        .collect()
}

fn candidate_indices_by_symbol(candidates: &[GraphCandidateSparsityInput]) -> Vec<Vec<usize>> {
    let mut group_by_symbol = HashMap::<&str, usize>::new();
    let mut groups = Vec::<Vec<usize>>::new();
    for (candidate_index, candidate) in candidates.iter().enumerate() {
        let next_group = groups.len();
        let group_index = *group_by_symbol
            .entry(candidate.symbol.as_str())
            .or_insert_with(|| {
                groups.push(Vec::new());
                next_group
            });
        groups[group_index].push(candidate_index);
    }
    groups
}
