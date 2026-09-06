// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-only

use std::collections::BTreeSet;

use crate::source_index::{ClientDbSourceIndexCandidate, ClientDbSourceIndexImport};

use super::source_index_query_scoring::source_index_structured_candidate_score;

/// Projects and ranks candidates from authoritative live source-snapshot facts.
///
/// This function owns candidate semantics for both live-memory and persisted
/// acquisition adapters. It performs no database or filesystem access.
pub(super) fn rank_live_source_index_candidates(
    import: &ClientDbSourceIndexImport,
    terms: &[String],
    language_id: Option<&str>,
    limit: u32,
) -> Vec<ClientDbSourceIndexCandidate> {
    if limit == 0 || terms.is_empty() {
        return Vec::new();
    }

    let mut seen_owner_paths = BTreeSet::new();
    let mut ranked = Vec::<(usize, ClientDbSourceIndexCandidate)>::new();
    for owner in &import.owners {
        if language_id.is_some_and(|requested| {
            owner.language_id.as_ref().map(|value| value.as_str()) != Some(requested)
        }) {
            continue;
        }
        if !seen_owner_paths.insert(owner.owner_path.as_str()) {
            continue;
        }

        let mut selector_haystack = String::new();
        let mut selector_symbol = None;
        let mut selector_kind = None;
        let mut selector_projection = None;
        for selector in import
            .selectors
            .iter()
            .filter(|selector| selector.owner_path == owner.owner_path)
        {
            selector_haystack.push(' ');
            selector_haystack.push_str(selector.selector_id.as_str());
            selector_haystack.push(' ');
            selector_haystack.push_str(selector.symbol.as_ref().map_or("", |value| value.as_str()));
            selector_haystack.push(' ');
            selector_haystack.push_str(selector.kind.as_ref().map_or("", |value| value.as_str()));
            selector_haystack.push(' ');
            selector_haystack.push_str(selector.source.as_str());
            for query_key in &selector.query_keys {
                selector_haystack.push(' ');
                selector_haystack.push_str(query_key.as_str());
            }
            if selector_projection.is_none() {
                selector_symbol = selector.symbol.clone();
                selector_kind = selector.kind.clone();
                selector_projection = Some(selector.projection_record.clone());
            }
        }

        let mut candidate = ClientDbSourceIndexCandidate::from(owner.clone());
        let score = source_index_structured_candidate_score(
            candidate.path.as_str(),
            candidate.language_id.as_ref().map(|value| value.as_str()),
            candidate.provider_id.as_ref().map(|value| value.as_str()),
            candidate.source_kind.as_str(),
            &candidate.query_keys,
            &selector_haystack,
            terms,
        );
        if score == 0 {
            continue;
        }
        candidate.selector_symbol = selector_symbol;
        candidate.selector_kind = selector_kind;
        candidate.selector_projection = selector_projection;
        ranked.push((score, candidate));
    }

    ranked.sort_by(|(left_score, left), (right_score, right)| {
        right_score
            .cmp(left_score)
            .then_with(|| left.path.cmp(&right.path))
    });
    ranked
        .into_iter()
        .map(|(_, candidate)| candidate)
        .take(limit as usize)
        .collect()
}
