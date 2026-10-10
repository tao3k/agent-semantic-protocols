// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Generation-derived working-memory admission for resident Search stages.

use std::collections::BTreeSet;

use crate::RuntimeQueryGeneration;
use crate::runtime_search_execution::RuntimeGrepMatch;

pub(super) fn retrieval_work_bytes(generation: &RuntimeQueryGeneration) -> usize {
    generation
        .resident()
        .resident_grep_corpus()
        .corpus_byte_count()
        .max(1)
}

pub(super) fn scoped_source_work_bytes(
    generation: &RuntimeQueryGeneration,
    owner_scope: &BTreeSet<String>,
) -> usize {
    let corpus = generation.resident().resident_grep_corpus();
    owner_scope
        .iter()
        .filter_map(|owner| corpus.owner_bytes(owner))
        .map(<[u8]>::len)
        .fold(0_usize, usize::saturating_add)
        .max(1)
}

pub(super) fn retrieval_working_memory_bytes(generation: &RuntimeQueryGeneration) -> usize {
    let corpus = generation.resident().resident_grep_corpus();
    let line_count = corpus
        .owner_spans
        .last()
        .map_or(0_u64, |span| span.end_line);
    let max_owner_path_bytes = corpus
        .owner_spans
        .iter()
        .map(|span| span.owner_path.len())
        .max()
        .unwrap_or(0);
    usize::try_from(line_count)
        .unwrap_or(usize::MAX)
        .saturating_mul(
            std::mem::size_of::<RuntimeGrepMatch>().saturating_add(max_owner_path_bytes),
        )
        .saturating_add(
            corpus
                .owner_spans
                .len()
                .saturating_mul(std::mem::size_of::<String>().saturating_mul(8)),
        )
        .max(1)
}

pub(super) fn structural_working_memory_bytes(
    generation: &RuntimeQueryGeneration,
    owner_scope: &BTreeSet<String>,
    match_count: usize,
) -> usize {
    let scoped_source_bytes = scoped_source_work_bytes(generation, owner_scope);
    scoped_source_bytes
        .saturating_add(match_count.saturating_mul(std::mem::size_of::<RuntimeGrepMatch>()))
        .max(1)
}

pub(super) fn graph_working_memory_bytes(
    generation: &RuntimeQueryGeneration,
    owner_scope: &BTreeSet<String>,
) -> usize {
    let scoped_source_bytes = structural_working_memory_bytes(generation, owner_scope, 0);
    let node_count = owner_scope.len().saturating_mul(2).max(1);
    let edge_count = node_count.saturating_mul(node_count).clamp(1, 1_024);
    scoped_source_bytes
        .saturating_add(node_count.saturating_mul(std::mem::size_of::<String>().saturating_mul(4)))
        .saturating_add(edge_count.saturating_mul(std::mem::size_of::<String>().saturating_mul(3)))
        .max(1)
}

pub(crate) fn project_topology_working_memory_bytes(
    source_descriptor_bytes: usize,
    node_count: usize,
    input_edge_count: usize,
    closure_edge_limit: usize,
) -> usize {
    let average_identity_bytes = source_descriptor_bytes
        .checked_div(node_count.max(1))
        .unwrap_or(0)
        .max(1);
    let node_bytes = node_count.saturating_mul(
        std::mem::size_of::<String>()
            .saturating_mul(3)
            .saturating_add(average_identity_bytes),
    );
    let edge_record_bytes = std::mem::size_of::<String>()
        .saturating_mul(3)
        .saturating_add(average_identity_bytes.saturating_mul(2));
    source_descriptor_bytes
        .saturating_add(node_bytes)
        .saturating_add(input_edge_count.saturating_mul(edge_record_bytes))
        .saturating_add(closure_edge_limit.saturating_mul(edge_record_bytes))
        .max(1)
}
