// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Immutable Tantivy delta layers for the resident workspace overlay.

use std::collections::BTreeSet;
use std::sync::Arc;

#[derive(Debug)]
pub(super) struct ResidentTantivyDeltaLayer {
    pub(super) shadowed_owner_paths: BTreeSet<String>,
    pub(super) index: Option<agent_semantic_search::ResidentTantivyDeltaIndex>,
    pub(super) previous: Option<Arc<ResidentTantivyDeltaLayer>>,
}

pub(super) fn merge_tantivy_owner_paths(
    head: Option<&ResidentTantivyDeltaLayer>,
    base_owner_paths: Vec<String>,
    expression: &str,
    language_id: &agent_semantic_client_core::LanguageId,
    admitted_owner_paths: Option<&[String]>,
    limit: usize,
) -> Result<Vec<String>, String> {
    let admitted = admitted_owner_paths.map(|paths| paths.iter().collect::<BTreeSet<_>>());
    let mut shadowed = BTreeSet::new();
    let mut seen = BTreeSet::new();
    let mut merged = Vec::new();
    let language_id = agent_semantic_config::LanguageId::new(language_id.as_str());
    let mut layer = head;
    while let Some(current) = layer {
        shadowed.extend(current.shadowed_owner_paths.iter().cloned());
        if let Some(index) = &current.index {
            for owner_path in
                index.query_language_owner_paths(expression, &language_id, limit.max(1))?
            {
                if admitted
                    .as_ref()
                    .is_none_or(|owners| owners.contains(&owner_path))
                    && seen.insert(owner_path.clone())
                {
                    merged.push(owner_path);
                    if merged.len() == limit {
                        return Ok(merged);
                    }
                }
            }
        }
        layer = current.previous.as_deref();
    }
    for owner_path in base_owner_paths {
        if !shadowed.contains(&owner_path)
            && admitted
                .as_ref()
                .is_none_or(|owners| owners.contains(&owner_path))
            && seen.insert(owner_path.clone())
        {
            merged.push(owner_path);
            if merged.len() == limit {
                break;
            }
        }
    }
    Ok(merged)
}
