// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Exact-selector carrier for ranked-text search.
//!
//! Owner membership is never promoted into parser item membership. Only
//! content-bound Symbol Skeleton hits enter this projection.

use std::collections::{BTreeMap, BTreeSet};

use crate::{WorkspaceSearchHitProjection, WorkspaceSearchSyntaxCandidate};

pub fn ranked_text_selector_candidates(
    expression: &str,
    owner_scope: &BTreeSet<String>,
    admitted_languages: &BTreeSet<&str>,
    hits: impl IntoIterator<Item = agent_semantic_symbol_index::SymbolSkeletonHitV1>,
    selector_limit: usize,
) -> Result<Vec<WorkspaceSearchSyntaxCandidate>, String> {
    let mut candidates = BTreeMap::new();
    for hit in hits {
        let Some(selector) = hit.structural_selector else {
            continue;
        };
        if !owner_scope.contains(&hit.owner_path)
            || hit
                .language_id
                .as_deref()
                .is_none_or(|language| !admitted_languages.contains(language))
        {
            continue;
        }
        candidates
            .entry(selector.clone())
            .or_insert_with(|| WorkspaceSearchSyntaxCandidate {
                owner: hit.owner_path,
                selector,
                relation: "native-parser:tantivy-symbol".to_owned(),
                hit: WorkspaceSearchHitProjection {
                    native: true,
                    tantivy: vec![expression.to_owned()],
                    ..Default::default()
                },
            });
        if candidates.len() > selector_limit {
            return Err(format!(
                "query-not-ready: Tantivy selector carrier budget exceeded: candidates={} limit={selector_limit}",
                candidates.len()
            ));
        }
    }
    Ok(candidates.into_values().collect())
}
