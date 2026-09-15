// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Generation-derived working-memory admission for resident Search stages.

use std::collections::BTreeSet;

use crate::RuntimeQueryGeneration;
use crate::runtime_resident_grep::RuntimeGrepMatch;

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
    let corpus = generation.resident().resident_grep_corpus();
    let scoped_source_bytes = owner_scope
        .iter()
        .filter_map(|owner| corpus.owner_bytes(owner))
        .map(<[u8]>::len)
        .fold(0_usize, usize::saturating_add);
    scoped_source_bytes
        .saturating_add(match_count.saturating_mul(std::mem::size_of::<RuntimeGrepMatch>()))
        .max(1)
}
