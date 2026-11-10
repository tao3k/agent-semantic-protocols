// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Persistent owner-byte overlay and content-search delta preparation.

use std::collections::BTreeSet;
use std::sync::Arc;

use super::{WorkspaceOwnerContentMutationV1, WorkspaceOwnerSnapshot};

#[derive(Debug)]
pub(crate) struct PreparedOwnerContentSearchDelta {
    pub(super) shadowed_owner_paths: BTreeSet<String>,
    pub(super) index: Option<agent_semantic_search::ResidentTantivyDeltaIndex>,
}

pub(crate) fn prepare_owner_content_search_delta(
    mutation: &WorkspaceOwnerContentMutationV1,
) -> Result<PreparedOwnerContentSearchDelta, String> {
    let mut documents = std::collections::BTreeMap::new();
    let mut shadowed_owner_paths = BTreeSet::new();
    for upsert in &mutation.upserts {
        let owner_path = upsert.owner.owner_path.clone();
        shadowed_owner_paths.insert(owner_path.clone());
        documents.insert(
            owner_path.clone(),
            agent_semantic_search::ResidentSourceDocument {
                owner_path: owner_path.clone(),
                owner_content_digest: upsert.owner.content_digest.clone(),
                line_count: u32::try_from(
                    upsert
                        .owner
                        .bytes
                        .iter()
                        .filter(|byte| **byte == b'\n')
                        .count()
                        + 1,
                )
                .unwrap_or(u32::MAX),
                query_keys: agent_semantic_search::resident_topology_coverage_features(
                    &owner_path,
                    std::iter::empty(),
                ),
                authority: upsert.owner.authority.clone(),
            },
        );
    }
    shadowed_owner_paths.extend(
        mutation
            .removals
            .iter()
            .map(|removal| removal.owner_path.clone()),
    );
    let index = if documents.is_empty() {
        None
    } else {
        Some(agent_semantic_search::ResidentTantivyDeltaIndex::new(
            documents,
            agent_semantic_search::ResidentIndexBuildResources::new(
                1,
                agent_semantic_search::ResidentIndexBuildResources::TANTIVY_MINIMUM_ARENA_BYTES_PER_THREAD,
                agent_semantic_search::ResidentIndexBuildStrategy::SingleSegmentBulk,
            )?,
        )?)
    };
    Ok(PreparedOwnerContentSearchDelta {
        shadowed_owner_paths,
        index,
    })
}

#[derive(Debug, Clone)]
pub(super) enum OwnerContentOverlayValue {
    Present {
        owner: Arc<WorkspaceOwnerSnapshot>,
        grams: BTreeSet<u32>,
    },
    Removed,
}

#[derive(Debug, Clone, Default)]
pub(super) struct OwnerContentOverlayTrie {
    root: Arc<OwnerContentOverlayNode>,
}

#[derive(Debug, Clone, Default)]
struct OwnerContentOverlayNode {
    value: Option<OwnerContentOverlayValue>,
    children: std::collections::BTreeMap<u8, Arc<OwnerContentOverlayNode>>,
}

impl OwnerContentOverlayTrie {
    pub(super) fn get(&self, owner_path: &str) -> Option<&OwnerContentOverlayValue> {
        let mut node = self.root.as_ref();
        for byte in owner_path.as_bytes() {
            node = node.children.get(byte)?.as_ref();
        }
        node.value.as_ref()
    }

    pub(super) fn with_updates(
        &self,
        updates: impl IntoIterator<Item = (String, OwnerContentOverlayValue)>,
    ) -> Self {
        let mut root = Arc::clone(&self.root);
        for (owner_path, value) in updates {
            root = owner_content_overlay_upsert(&root, owner_path.as_bytes(), value);
        }
        Self { root }
    }

    pub(super) fn entries(&self) -> Vec<(String, &OwnerContentOverlayValue)> {
        fn visit<'a>(
            node: &'a OwnerContentOverlayNode,
            path: &mut Vec<u8>,
            output: &mut Vec<(String, &'a OwnerContentOverlayValue)>,
        ) {
            if let Some(value) = &node.value
                && let Ok(owner_path) = std::str::from_utf8(path)
            {
                output.push((owner_path.to_owned(), value));
            }
            for (byte, child) in &node.children {
                path.push(*byte);
                visit(child, path, output);
                path.pop();
            }
        }
        let mut output = Vec::new();
        visit(&self.root, &mut Vec::new(), &mut output);
        output
    }
}

fn owner_content_overlay_upsert(
    node: &Arc<OwnerContentOverlayNode>,
    suffix: &[u8],
    value: OwnerContentOverlayValue,
) -> Arc<OwnerContentOverlayNode> {
    let mut next = node.as_ref().clone();
    if let Some((byte, rest)) = suffix.split_first() {
        let child = next.children.get(byte).cloned().unwrap_or_default();
        next.children
            .insert(*byte, owner_content_overlay_upsert(&child, rest, value));
    } else {
        next.value = Some(value);
    }
    Arc::new(next)
}
