// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Owner-local Topology Index preparation and immutable delta traversal.

use std::collections::{BTreeSet, HashSet};
use std::sync::Arc;

use super::resident_content_overlay::OwnerContentOverlayValue;
use super::{
    ResidentOverlaySnapshot, WorkspaceMemoryGeneration, WorkspaceOwnerSnapshot,
    WorkspaceOwnerTopologyRebindV1, WorkspaceTopologyHit,
};

#[derive(Debug)]
pub(super) struct ResidentTopologyDeltaLayer {
    pub(super) shadowed_owner_paths: BTreeSet<String>,
    pub(super) index: Option<agent_semantic_topology::TopologyIndexV1>,
    pub(super) previous: Option<Arc<ResidentTopologyDeltaLayer>>,
}

#[derive(Debug)]
pub(crate) struct PreparedOwnerTopologyRebind {
    pub(super) shadowed_owner_paths: BTreeSet<String>,
    pub(super) topology_index: agent_semantic_topology::TopologyIndexV1,
    pub(super) tantivy_index: agent_semantic_search::ResidentTantivyDeltaIndex,
    pub(super) topology_node_count: usize,
}

pub(crate) fn prepare_owner_topology_rebind(
    rebind: &WorkspaceOwnerTopologyRebindV1,
) -> Result<PreparedOwnerTopologyRebind, String> {
    rebind.validate()?;
    let shadowed_owner_paths = rebind
        .owners
        .iter()
        .map(|owner| owner.owner_path.clone())
        .collect();
    let topology_owners = rebind
        .owners
        .iter()
        .map(workspace_owner_topology)
        .collect::<Result<Vec<_>, _>>()?;
    let topology_index = agent_semantic_topology::TopologyIndexV1::build(topology_owners)?;
    let topology_node_count = topology_index.node_count();
    let tantivy_documents = rebind
        .owners
        .iter()
        .map(|owner| {
            (
                owner.owner_path.clone(),
                workspace_owner_topology_document(owner),
            )
        })
        .collect();
    let tantivy_index = agent_semantic_search::ResidentTantivyDeltaIndex::new(
        tantivy_documents,
        agent_semantic_search::ResidentIndexBuildResources::new(
            1,
            agent_semantic_search::ResidentIndexBuildResources::TANTIVY_MINIMUM_ARENA_BYTES_PER_THREAD,
            agent_semantic_search::ResidentIndexBuildStrategy::SingleSegmentBulk,
        )?,
    )?;
    Ok(PreparedOwnerTopologyRebind {
        shadowed_owner_paths,
        topology_index,
        tantivy_index,
        topology_node_count,
    })
}

pub(super) fn merge_topology_hits(
    head: Option<&ResidentTopologyDeltaLayer>,
    base_hits: Vec<WorkspaceTopologyHit>,
    query: &str,
    limit: usize,
) -> Vec<WorkspaceTopologyHit> {
    let mut shadowed = BTreeSet::new();
    let mut seen = HashSet::new();
    let mut merged = Vec::new();
    let mut layer = head;
    while let Some(current) = layer {
        shadowed.extend(current.shadowed_owner_paths.iter().cloned());
        if let Some(index) = &current.index {
            append_unique_hits(
                &mut merged,
                &mut seen,
                index.query(query, limit.max(1)),
                limit,
            );
            if merged.len() == limit {
                return merged;
            }
        }
        layer = current.previous.as_deref();
    }
    append_unique_hits(
        &mut merged,
        &mut seen,
        base_hits
            .into_iter()
            .filter(|hit| !shadowed.contains(&hit.owner_path)),
        limit,
    );
    merged
}

pub(super) fn resolve_topology_anchor(
    head: Option<&ResidentTopologyDeltaLayer>,
    owner_path: &str,
    owner_content_digest: &str,
    match_start: usize,
    match_end: usize,
    base: impl FnOnce() -> Result<Option<agent_semantic_topology::TopologyAnchorHitV1>, String>,
) -> Result<Option<agent_semantic_topology::TopologyAnchorHitV1>, String> {
    let mut layer = head;
    while let Some(current) = layer {
        if current.shadowed_owner_paths.contains(owner_path) {
            return current.index.as_ref().map_or(Ok(None), |index| {
                index.smallest_enclosing_anchor(
                    owner_path,
                    owner_content_digest,
                    match_start,
                    match_end,
                )
            });
        }
        layer = current.previous.as_deref();
    }
    base()
}

pub(super) fn resolve_topology_selector(
    head: Option<&ResidentTopologyDeltaLayer>,
    owner_path: &str,
    selector: &str,
    base: impl FnOnce() -> Option<WorkspaceTopologyHit>,
) -> Option<WorkspaceTopologyHit> {
    let mut layer = head;
    while let Some(current) = layer {
        if current.shadowed_owner_paths.contains(owner_path) {
            return current
                .index
                .as_ref()
                .and_then(|index| index.exact_selector(selector));
        }
        layer = current.previous.as_deref();
    }
    base()
}

impl ResidentOverlaySnapshot {
    pub(super) fn resolve_topology_selector(
        &self,
        owner_path: &str,
        selector: &str,
        base: impl FnOnce() -> Option<WorkspaceTopologyHit>,
    ) -> Option<WorkspaceTopologyHit> {
        resolve_topology_selector(
            self.state.topology_delta_head.as_deref(),
            owner_path,
            selector,
            base,
        )
    }

    pub(super) fn resolve_topology_anchor(
        &self,
        owner_path: &str,
        owner_content_digest: &str,
        match_start: usize,
        match_end: usize,
        base: impl FnOnce() -> Result<Option<agent_semantic_topology::TopologyAnchorHitV1>, String>,
    ) -> Result<Option<agent_semantic_topology::TopologyAnchorHitV1>, String> {
        resolve_topology_anchor(
            self.state.topology_delta_head.as_deref(),
            owner_path,
            owner_content_digest,
            match_start,
            match_end,
            base,
        )
    }

    pub(super) fn owner_content_digest<'a>(
        &'a self,
        base: &'a WorkspaceMemoryGeneration,
        owner_path: &str,
    ) -> Option<&'a str> {
        if let Some(content) = self.state.content_owners.get(owner_path) {
            return match content {
                OwnerContentOverlayValue::Present { owner, .. } => {
                    Some(owner.content_digest.as_str())
                }
                OwnerContentOverlayValue::Removed => None,
            };
        }
        if self.state.tombstones.contains(owner_path) {
            return None;
        }
        self.state
            .owners
            .get(owner_path)
            .map(|owner| owner.content_digest.as_str())
            .or_else(|| {
                base.owners
                    .iter()
                    .find(|owner| owner.owner_path == owner_path)
                    .map(|owner| owner.content_digest.as_str())
            })
    }
}

fn append_unique_hits(
    merged: &mut Vec<WorkspaceTopologyHit>,
    seen: &mut HashSet<(String, String)>,
    hits: impl IntoIterator<Item = WorkspaceTopologyHit>,
    limit: usize,
) {
    for hit in hits {
        let key = (hit.owner_path.clone(), hit.topology_locator.clone());
        if seen.insert(key) {
            merged.push(hit);
            if merged.len() == limit {
                break;
            }
        }
    }
}

fn workspace_owner_topology(
    owner: &WorkspaceOwnerSnapshot,
) -> Result<agent_semantic_topology::TopologyOwnerV1, String> {
    let nodes = owner
        .selectors
        .iter()
        .filter(|selector| !selector.query_keys.is_empty())
        .map(|selector| {
            agent_semantic_topology::TopologyNodeV1::from_selector_with_anchor(
                selector.selector.clone(),
                selector.query_keys.clone(),
                selector.byte_start,
                selector.byte_end,
            )
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(agent_semantic_topology::TopologyOwnerV1 {
        owner_path: owner.owner_path.clone(),
        owner_content_digest: owner.content_digest.clone(),
        nodes,
    })
}

fn workspace_owner_topology_document(
    owner: &WorkspaceOwnerSnapshot,
) -> agent_semantic_search::ResidentSourceDocument {
    agent_semantic_search::ResidentSourceDocument {
        owner_path: owner.owner_path.clone(),
        owner_content_digest: owner.content_digest.clone(),
        line_count: u32::try_from(owner.bytes.iter().filter(|byte| **byte == b'\n').count() + 1)
            .unwrap_or(u32::MAX),
        query_keys: agent_semantic_search::resident_topology_coverage_features(
            &owner.owner_path,
            owner
                .selectors
                .iter()
                .flat_map(|selector| selector.query_keys.iter().cloned()),
        ),
        authority: owner.authority.clone(),
    }
}
