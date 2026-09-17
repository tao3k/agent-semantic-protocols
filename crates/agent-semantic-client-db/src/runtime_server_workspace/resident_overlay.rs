// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Millisecond resident owner and selector overlays behind one writer lane.

use std::collections::{BTreeSet, HashMap, HashSet};
use std::sync::Arc;

use parking_lot::RwLock;

use super::{
    WorkspaceMemoryGeneration, WorkspaceOwnerSnapshot, WorkspaceRuntimeSelectorOverlay,
    WorkspaceRuntimeSelectorOverlayReceipt, WorkspaceRuntimeSelectorRead,
};

#[derive(Debug)]
pub(crate) struct ResidentOverlayStore {
    state: RwLock<ResidentOverlayState>,
}

#[derive(Debug, Clone)]
struct ResidentOverlayState {
    base_generation_digest: String,
    generation_digest: String,
    revision: u64,
    workspace_snapshot: Arc<agent_semantic_content_identity::WorkspaceSnapshot>,
    owner_identity_tree: Arc<
        agent_semantic_content_identity::workspace_merkle_v1::WorkspacePathMerkleTreeIncrementalV1,
    >,
    content_owners: OwnerContentOverlayTrie,
    tantivy_delta_head: Option<Arc<ResidentTantivyDeltaLayer>>,
    symbol_delta_head: Option<Arc<ResidentSymbolDeltaLayer>>,
    owners: Arc<HashMap<String, WorkspaceOwnerSnapshot>>,
    relations: Arc<HashMap<String, Vec<crate::ClientDbSourceIndexOwnedRelation>>>,
    tombstones: Arc<HashSet<String>>,
    semantic_owners: Arc<HashSet<String>>,
    owner_grams: Arc<HashMap<String, BTreeSet<u32>>>,
    selectors:
        Arc<HashMap<(super::model::ExactProjectionKind, String), WorkspaceRuntimeSelectorOverlay>>,
}

#[derive(Debug)]
struct ResidentTantivyDeltaLayer {
    shadowed_owner_paths: BTreeSet<String>,
    index: Option<agent_semantic_search::ResidentTantivyDeltaIndex>,
    previous: Option<Arc<ResidentTantivyDeltaLayer>>,
}

#[derive(Debug)]
struct ResidentSymbolDeltaLayer {
    shadowed_owner_paths: BTreeSet<String>,
    index: Option<agent_semantic_symbol_index::SymbolSkeletonIndexV1>,
    previous: Option<Arc<ResidentSymbolDeltaLayer>>,
}

#[derive(Debug)]
pub(crate) struct PreparedOwnerContentSearchDelta {
    shadowed_owner_paths: BTreeSet<String>,
    index: Option<agent_semantic_search::ResidentTantivyDeltaIndex>,
}

#[derive(Debug)]
pub(crate) struct PreparedOwnerSymbolRebind {
    shadowed_owner_paths: BTreeSet<String>,
    symbol_index: agent_semantic_symbol_index::SymbolSkeletonIndexV1,
    tantivy_index: agent_semantic_search::ResidentTantivyDeltaIndex,
    symbol_count: usize,
}

pub(crate) fn prepare_owner_symbol_rebind(
    rebind: &super::WorkspaceOwnerSymbolRebindV1,
) -> Result<PreparedOwnerSymbolRebind, String> {
    rebind.validate()?;
    let shadowed_owner_paths = rebind
        .owners
        .iter()
        .map(|owner| owner.owner_path.clone())
        .collect();
    let symbol_index = agent_semantic_symbol_index::SymbolSkeletonIndexV1::build(
        rebind.owners.iter().map(workspace_owner_symbol_skeleton),
    )?;
    let symbol_count = symbol_index.symbol_count();
    let tantivy_documents = rebind
        .owners
        .iter()
        .map(|owner| {
            (
                owner.owner_path.clone(),
                workspace_owner_symbol_document(owner),
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
    Ok(PreparedOwnerSymbolRebind {
        shadowed_owner_paths,
        symbol_index,
        tantivy_index,
        symbol_count,
    })
}

pub(crate) fn prepare_owner_content_search_delta(
    mutation: &super::WorkspaceOwnerContentMutationV1,
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
                query_keys: agent_semantic_search::resident_skeleton_coverage_keys(
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
enum OwnerContentOverlayValue {
    Present {
        owner: Arc<WorkspaceOwnerSnapshot>,
        grams: BTreeSet<u32>,
    },
    Removed,
}

#[derive(Debug, Clone, Default)]
struct OwnerContentOverlayTrie {
    root: Arc<OwnerContentOverlayNode>,
}

#[derive(Debug, Clone, Default)]
struct OwnerContentOverlayNode {
    value: Option<OwnerContentOverlayValue>,
    children: std::collections::BTreeMap<u8, Arc<OwnerContentOverlayNode>>,
}

impl OwnerContentOverlayTrie {
    fn get(&self, owner_path: &str) -> Option<&OwnerContentOverlayValue> {
        let mut node = self.root.as_ref();
        for byte in owner_path.as_bytes() {
            node = node.children.get(byte)?.as_ref();
        }
        node.value.as_ref()
    }

    fn with_updates(
        &self,
        updates: impl IntoIterator<Item = (String, OwnerContentOverlayValue)>,
    ) -> Self {
        let mut root = Arc::clone(&self.root);
        for (owner_path, value) in updates {
            root = owner_content_overlay_upsert(&root, owner_path.as_bytes(), value);
        }
        Self { root }
    }

    fn entries(&self) -> Vec<(String, &OwnerContentOverlayValue)> {
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

#[derive(Debug, Clone)]
pub(crate) struct ResidentOverlaySnapshot {
    state: ResidentOverlayState,
}

impl ResidentOverlayStore {
    pub(super) fn new(generation: Option<&WorkspaceMemoryGeneration>) -> Self {
        let state = generation.map_or_else(ResidentOverlayState::empty, ResidentOverlayState::new);
        Self {
            state: RwLock::new(state),
        }
    }

    pub(super) fn reset(&self, generation: &WorkspaceMemoryGeneration) {
        *self.state.write() = ResidentOverlayState::new(generation);
    }

    pub(super) fn commit(&self, snapshot: ResidentOverlaySnapshot) {
        *self.state.write() = snapshot.state;
    }

    pub(super) fn snapshot(&self, base: &WorkspaceMemoryGeneration) -> ResidentOverlaySnapshot {
        let state = self.state.read();
        if state.base_generation_digest == base.generation_digest {
            ResidentOverlaySnapshot {
                state: state.clone(),
            }
        } else {
            ResidentOverlaySnapshot {
                state: ResidentOverlayState::new(base),
            }
        }
    }

    pub(super) fn publish_owner(
        &self,
        base: &WorkspaceMemoryGeneration,
        owner: WorkspaceOwnerSnapshot,
    ) -> Result<ResidentOverlaySnapshot, String> {
        validate_owner(&owner)?;
        let mut state = self.staged_state(base);
        let owner_path = owner.owner_path.clone();
        let owner_digest = owner.content_digest.clone();
        Arc::make_mut(&mut state.owner_grams)
            .insert(owner_path.clone(), owner_byte_grams(&owner.bytes));
        Arc::make_mut(&mut state.owners).insert(owner_path.clone(), owner);
        Arc::make_mut(&mut state.relations).insert(owner_path.clone(), Vec::new());
        Arc::make_mut(&mut state.semantic_owners).remove(&owner_path);
        Arc::make_mut(&mut state.tombstones).remove(&owner_path);
        Arc::make_mut(&mut state.selectors).retain(|_, selector| selector.owner_path != owner_path);
        state.workspace_snapshot = Arc::new(
            state
                .workspace_snapshot
                .with_overlay([(owner_path, owner_digest)]),
        );
        state.advance();
        Ok(ResidentOverlaySnapshot { state })
    }

    pub(super) fn publish_owner_delta(
        &self,
        base: &WorkspaceMemoryGeneration,
        owners: Vec<WorkspaceOwnerSnapshot>,
        tombstones: Vec<String>,
        relations: Vec<crate::ClientDbSourceIndexOwnedRelation>,
    ) -> Result<ResidentOverlaySnapshot, String> {
        if owners.is_empty() && tombstones.is_empty() {
            return Err("runtime owner delta must not be empty".to_owned());
        }
        let mut state = self.staged_state(base);
        let mut changed = std::collections::BTreeMap::new();
        let mut removed = std::collections::BTreeSet::new();
        for owner in owners {
            validate_owner(&owner)?;
            let owner_path = owner.owner_path.clone();
            if removed.contains(&owner_path)
                || changed
                    .insert(owner_path.clone(), owner.content_digest.clone())
                    .is_some()
            {
                return Err(format!(
                    "runtime owner delta contains a duplicate owner: {owner_path}"
                ));
            }
            Arc::make_mut(&mut state.owner_grams)
                .insert(owner_path.clone(), owner_byte_grams(&owner.bytes));
            Arc::make_mut(&mut state.owners).insert(owner_path.clone(), owner);
            Arc::make_mut(&mut state.relations).insert(owner_path.clone(), Vec::new());
            Arc::make_mut(&mut state.semantic_owners).remove(&owner_path);
            Arc::make_mut(&mut state.tombstones).remove(&owner_path);
            Arc::make_mut(&mut state.selectors)
                .retain(|_, selector| selector.owner_path != owner_path);
        }
        for owner_path in tombstones {
            if changed.contains_key(&owner_path) || !removed.insert(owner_path.clone()) {
                return Err(format!(
                    "runtime owner delta contains a duplicate owner: {owner_path}"
                ));
            }
            if Arc::make_mut(&mut state.owners)
                .remove(&owner_path)
                .is_none()
                && base_owner(base, &owner_path).is_none()
            {
                return Err(format!(
                    "runtime owner tombstone target is unavailable: {owner_path}"
                ));
            }
            Arc::make_mut(&mut state.tombstones).insert(owner_path.clone());
            Arc::make_mut(&mut state.relations).remove(&owner_path);
            Arc::make_mut(&mut state.owner_grams).remove(&owner_path);
            Arc::make_mut(&mut state.semantic_owners).remove(&owner_path);
            Arc::make_mut(&mut state.selectors)
                .retain(|_, selector| selector.owner_path != owner_path);
        }
        for relation in relations {
            let owner_path = relation.owner_path.as_str();
            if !changed.contains_key(owner_path) {
                return Err(format!(
                    "runtime owner delta relation is outside changed owner membership: {owner_path}"
                ));
            }
            relation.relation.validate()?;
            Arc::make_mut(&mut state.relations)
                .get_mut(owner_path)
                .expect("changed owner relation bucket was initialized")
                .push(relation);
        }
        state.workspace_snapshot = Arc::new(
            state
                .workspace_snapshot
                .with_overlay_delta(changed, removed),
        );
        state.advance();
        Ok(ResidentOverlaySnapshot { state })
    }

    pub(super) fn publish_content_owner_mutation(
        &self,
        base: &WorkspaceMemoryGeneration,
        mutation: super::WorkspaceOwnerContentMutationV1,
        prepared_search_delta: PreparedOwnerContentSearchDelta,
    ) -> Result<
        (
            ResidentOverlaySnapshot,
            agent_semantic_content_identity::workspace_merkle_v1::WorkspaceMerkleDeltaMetricsIncrementalV1,
        ),
        String,
    >{
        mutation.validate()?;
        if mutation.base_generation_digest != base.generation_digest {
            return Err(
                "workspace owner content mutation base generation digest mismatch".to_owned(),
            );
        }
        let mut state = self.staged_state(base);
        let mut operations = Vec::with_capacity(mutation.upserts.len() + mutation.removals.len());
        let mut content_updates =
            Vec::with_capacity(mutation.upserts.len() + mutation.removals.len());
        for upsert in mutation.upserts {
            let owner_path = upsert.owner.owner_path.clone();
            let current = state.owner_snapshot(base, &owner_path);
            if current.as_ref().map(|owner| owner.content_digest.as_str())
                != upsert.previous_content_digest.as_deref()
            {
                return Err(format!(
                    "workspace owner content mutation previous digest mismatch: {owner_path}"
                ));
            }
            if current.as_ref().and_then(|owner| owner.authority.as_ref())
                != upsert.owner.authority.as_ref()
            {
                return Err(format!(
                    "workspace owner content mutation authority drift: {owner_path}"
                ));
            }
            operations.push(
                agent_semantic_content_identity::workspace_merkle_v1::WorkspaceMerkleDeltaOperationIncrementalV1::Upsert {
                    owner_path: owner_path.clone(),
                    previous_source_blob_digest: upsert
                        .previous_content_digest
                        .as_deref()
                        .map(parse_typed_content_digest)
                        .transpose()?,
                    source_blob_digest: parse_typed_content_digest(&upsert.owner.content_digest)?,
                },
            );
            let grams = owner_byte_grams(&upsert.owner.bytes);
            content_updates.push((
                owner_path,
                OwnerContentOverlayValue::Present {
                    owner: Arc::new(upsert.owner),
                    grams,
                },
            ));
        }
        for removal in mutation.removals {
            let owner_path = removal.owner_path;
            let current = state.owner_snapshot(base, &owner_path).ok_or_else(|| {
                format!("workspace owner content mutation removal is missing: {owner_path}")
            })?;
            if current.content_digest != removal.previous_content_digest {
                return Err(format!(
                    "workspace owner content mutation previous digest mismatch: {owner_path}"
                ));
            }
            operations.push(
                agent_semantic_content_identity::workspace_merkle_v1::WorkspaceMerkleDeltaOperationIncrementalV1::Remove {
                    owner_path: owner_path.clone(),
                    previous_source_blob_digest: parse_typed_content_digest(
                        &removal.previous_content_digest,
                    )?,
                },
            );
            content_updates.push((owner_path, OwnerContentOverlayValue::Removed));
        }
        let (owner_identity_tree, metrics) = state
            .owner_identity_tree
            .apply_delta(&operations)
            .map_err(|error| format!("apply owner-local content identity delta: {error:?}"))?;
        state.owner_identity_tree = Arc::new(owner_identity_tree);
        state.content_owners = state.content_owners.with_updates(content_updates);
        let shadowed_owner_paths = prepared_search_delta.shadowed_owner_paths;
        state.tantivy_delta_head = Some(Arc::new(ResidentTantivyDeltaLayer {
            shadowed_owner_paths: shadowed_owner_paths.clone(),
            index: prepared_search_delta.index,
            previous: state.tantivy_delta_head.clone(),
        }));
        state.symbol_delta_head = Some(Arc::new(ResidentSymbolDeltaLayer {
            shadowed_owner_paths,
            index: None,
            previous: state.symbol_delta_head.clone(),
        }));
        state.advance_owner_content_mutation(&operations);
        Ok((ResidentOverlaySnapshot { state }, metrics))
    }

    pub(super) fn publish_owner_symbol_rebind(
        &self,
        base: &WorkspaceMemoryGeneration,
        rebind: super::WorkspaceOwnerSymbolRebindV1,
        prepared: PreparedOwnerSymbolRebind,
    ) -> Result<(ResidentOverlaySnapshot, usize), String> {
        rebind.validate()?;
        let owners = rebind.owners;
        let relations = rebind.relations;
        let mut state = self.staged_state(base);
        let mut changed = HashSet::with_capacity(owners.len());
        for owner in &owners {
            validate_owner(owner)?;
            if !changed.insert(owner.owner_path.as_str()) {
                return Err(format!(
                    "runtime semantic owner delta contains a duplicate owner: {}",
                    owner.owner_path
                ));
            }
            let current = state
                .owner_snapshot(base, &owner.owner_path)
                .ok_or_else(|| {
                    format!(
                        "runtime semantic owner is outside canonical content identity: {}",
                        owner.owner_path
                    )
                })?;
            if current.content_digest != owner.content_digest || current.bytes != owner.bytes {
                return Err(format!(
                    "runtime semantic owner content identity drift: {}",
                    owner.owner_path
                ));
            }
        }
        for relation in &relations {
            if !changed.contains(relation.owner_path.as_str()) {
                return Err(format!(
                    "runtime semantic owner delta relation is outside changed owner membership: {}",
                    relation.owner_path
                ));
            }
            relation.relation.validate()?;
        }

        for owner in owners {
            let owner_path = owner.owner_path.clone();
            Arc::make_mut(&mut state.owner_grams)
                .insert(owner_path.clone(), owner_byte_grams(&owner.bytes));
            Arc::make_mut(&mut state.owners).insert(owner_path.clone(), owner);
            Arc::make_mut(&mut state.relations).insert(owner_path.clone(), Vec::new());
            Arc::make_mut(&mut state.semantic_owners).insert(owner_path.clone());
            Arc::make_mut(&mut state.tombstones).remove(&owner_path);
            Arc::make_mut(&mut state.selectors)
                .retain(|_, selector| selector.owner_path != owner_path);
        }
        for relation in relations {
            Arc::make_mut(&mut state.relations)
                .get_mut(relation.owner_path.as_str())
                .expect("semantic owner relation bucket was initialized")
                .push(relation);
        }
        state.tantivy_delta_head = Some(Arc::new(ResidentTantivyDeltaLayer {
            shadowed_owner_paths: prepared.shadowed_owner_paths.clone(),
            index: Some(prepared.tantivy_index),
            previous: state.tantivy_delta_head.clone(),
        }));
        state.symbol_delta_head = Some(Arc::new(ResidentSymbolDeltaLayer {
            shadowed_owner_paths: prepared.shadowed_owner_paths,
            index: Some(prepared.symbol_index),
            previous: state.symbol_delta_head.clone(),
        }));
        // Parser projection changes semantic membership, not source content.
        // Keep the canonical Merkle snapshot shared and advance this transaction once.
        state.advance();
        Ok((ResidentOverlaySnapshot { state }, prepared.symbol_count))
    }

    pub(super) fn tombstone_owner(
        &self,
        base: &WorkspaceMemoryGeneration,
        owner_path: &str,
    ) -> Result<ResidentOverlaySnapshot, String> {
        let mut state = self.staged_state(base);
        if Arc::make_mut(&mut state.owners)
            .remove(owner_path)
            .is_none()
            && base_owner(base, owner_path).is_none()
        {
            return Err("runtime owner tombstone target is unavailable".to_owned());
        }
        Arc::make_mut(&mut state.tombstones).insert(owner_path.to_owned());
        Arc::make_mut(&mut state.relations).remove(owner_path);
        Arc::make_mut(&mut state.owner_grams).remove(owner_path);
        Arc::make_mut(&mut state.semantic_owners).remove(owner_path);
        Arc::make_mut(&mut state.selectors).retain(|_, selector| selector.owner_path != owner_path);
        state.workspace_snapshot = Arc::new(state.workspace_snapshot.with_overlay_delta(
            std::iter::empty::<(String, String)>(),
            [owner_path.to_owned()],
        ));
        state.advance();
        Ok(ResidentOverlaySnapshot { state })
    }

    pub(super) fn relocate_owner(
        &self,
        base: &WorkspaceMemoryGeneration,
        previous_owner_path: &str,
        owner: WorkspaceOwnerSnapshot,
    ) -> Result<ResidentOverlaySnapshot, String> {
        if previous_owner_path == owner.owner_path {
            return Err("runtime owner relocation requires two distinct owner paths".to_owned());
        }
        validate_owner(&owner)?;
        let mut state = self.staged_state(base);
        if Arc::make_mut(&mut state.owners)
            .remove(previous_owner_path)
            .is_none()
            && base_owner(base, previous_owner_path).is_none()
        {
            return Err("runtime owner relocation source is unavailable".to_owned());
        }
        Arc::make_mut(&mut state.tombstones).insert(previous_owner_path.to_owned());
        Arc::make_mut(&mut state.owner_grams).remove(previous_owner_path);
        Arc::make_mut(&mut state.semantic_owners).remove(previous_owner_path);
        Arc::make_mut(&mut state.selectors)
            .retain(|_, selector| selector.owner_path != previous_owner_path);
        let owner_path = owner.owner_path.clone();
        let owner_digest = owner.content_digest.clone();
        Arc::make_mut(&mut state.owner_grams)
            .insert(owner_path.clone(), owner_byte_grams(&owner.bytes));
        Arc::make_mut(&mut state.owners).insert(owner_path.clone(), owner);
        Arc::make_mut(&mut state.semantic_owners).remove(&owner_path);
        Arc::make_mut(&mut state.tombstones).remove(&owner_path);
        state.workspace_snapshot = Arc::new(state.workspace_snapshot.with_overlay_delta(
            [(owner_path, owner_digest)],
            [previous_owner_path.to_owned()],
        ));
        state.advance();
        Ok(ResidentOverlaySnapshot { state })
    }

    pub(super) fn publish_selector(
        &self,
        base: &WorkspaceMemoryGeneration,
        workspace_identity: &str,
        overlay: WorkspaceRuntimeSelectorOverlay,
    ) -> Result<
        (
            WorkspaceRuntimeSelectorOverlayReceipt,
            Option<ResidentOverlaySnapshot>,
        ),
        String,
    > {
        if selector_owner_path(&overlay.structural_selector)? != overlay.owner_path {
            return Err("runtime selector overlay owner path mismatch".to_owned());
        }
        let mut state = self.staged_state(base);
        let owner = if state.tombstones.contains(&overlay.owner_path) {
            None
        } else {
            state
                .owners
                .get(&overlay.owner_path)
                .or_else(|| base_owner(base, &overlay.owner_path))
        }
        .ok_or_else(|| "runtime selector overlay owner is unavailable".to_owned())?;
        validate_selector_overlay(owner, &base.projection_capability, &overlay)?;
        let key = (overlay.projection_kind, overlay.structural_selector.clone());
        let inserted = match state.selectors.get(&key) {
            Some(existing) if existing == &overlay => false,
            Some(_) => {
                return Err(
                    "runtime selector overlay conflicts with an admitted projection".to_owned(),
                );
            }
            None => {
                Arc::make_mut(&mut state.selectors).insert(key, overlay.clone());
                state.advance();
                true
            }
        };
        let receipt = WorkspaceRuntimeSelectorOverlayReceipt {
            schema_id: super::model::WORKSPACE_RUNTIME_SELECTOR_OVERLAY_RECEIPT_SCHEMA_ID
                .to_owned(),
            schema_version: "1".to_owned(),
            workspace_identity: workspace_identity.to_owned(),
            generation_digest: state.generation_digest.clone(),
            projection_kind: overlay.projection_kind,
            structural_selector: overlay.structural_selector,
            inserted,
        };
        Ok((
            receipt,
            inserted.then_some(ResidentOverlaySnapshot { state }),
        ))
    }

    pub(super) fn rebind_selector(
        &self,
        base: &WorkspaceMemoryGeneration,
        workspace_identity: &str,
        rebind: super::model::WorkspaceRuntimeSelectorRebind,
    ) -> Result<
        (
            WorkspaceRuntimeSelectorOverlayReceipt,
            Option<ResidentOverlaySnapshot>,
        ),
        String,
    > {
        let super::model::WorkspaceRuntimeSelectorRebind { owner, overlay } = rebind;
        if owner.owner_path != overlay.owner_path
            || owner.content_digest != overlay.owner_content_digest
        {
            return Err("runtime selector rebind live owner identity mismatch".to_owned());
        }
        validate_owner(&owner)?;
        let mut state = self.staged_state(base);
        let owner_path = owner.owner_path.clone();
        let owner_digest = owner.content_digest.clone();
        Arc::make_mut(&mut state.owner_grams)
            .insert(owner_path.clone(), owner_byte_grams(&owner.bytes));
        Arc::make_mut(&mut state.owners).insert(owner_path.clone(), owner.clone());
        Arc::make_mut(&mut state.semantic_owners).insert(owner_path.clone());
        Arc::make_mut(&mut state.tombstones).remove(&owner_path);
        Arc::make_mut(&mut state.selectors).retain(|_, selector| selector.owner_path != owner_path);
        state.workspace_snapshot = Arc::new(
            state
                .workspace_snapshot
                .with_overlay([(owner_path, owner_digest)]),
        );
        validate_selector_overlay(&owner, &base.projection_capability, &overlay)?;
        let key = (overlay.projection_kind, overlay.structural_selector.clone());
        Arc::make_mut(&mut state.selectors).insert(key, overlay.clone());
        state.advance();
        let receipt = WorkspaceRuntimeSelectorOverlayReceipt {
            schema_id: super::model::WORKSPACE_RUNTIME_SELECTOR_OVERLAY_RECEIPT_SCHEMA_ID
                .to_owned(),
            schema_version: "1".to_owned(),
            workspace_identity: workspace_identity.to_owned(),
            generation_digest: state.generation_digest.clone(),
            projection_kind: overlay.projection_kind,
            structural_selector: overlay.structural_selector,
            inserted: true,
        };
        Ok((receipt, Some(ResidentOverlaySnapshot { state })))
    }

    fn staged_state(&self, base: &WorkspaceMemoryGeneration) -> ResidentOverlayState {
        let state = self.state.read();
        if state.base_generation_digest == base.generation_digest {
            state.clone()
        } else {
            ResidentOverlayState::new(base)
        }
    }
}

impl ResidentOverlaySnapshot {
    pub(super) fn owner_paths_for_graph_entry_node_ids<'a>(
        &self,
        base: &WorkspaceMemoryGeneration,
        node_ids: impl IntoIterator<Item = &'a str>,
    ) -> BTreeSet<String> {
        let requested = node_ids.into_iter().collect::<HashSet<_>>();
        self.state
            .owners
            .values()
            .filter(|owner| !self.state.tombstones.contains(&owner.owner_path))
            .filter(|owner| {
                requested.contains(
                    agent_semantic_search::stable_graph_node_id("owner", &owner.owner_path)
                        .as_str(),
                ) || owner.selectors.iter().any(|selector| {
                    requested.contains(
                        agent_semantic_search::stable_graph_node_id("item", &selector.selector)
                            .as_str(),
                    )
                })
            })
            .map(|owner| owner.owner_path.clone())
            .filter(|owner| self.owner_snapshot(base, owner).is_some())
            .collect()
    }

    pub(super) fn materialize_generation(
        &self,
        _base: &WorkspaceMemoryGeneration,
        _projection_capability: crate::active_generation_projection_capability::ActiveGenerationProjectionCapabilityManifest,
    ) -> Result<WorkspaceMemoryGeneration, String> {
        Err(
            "query-not-ready: resident overlay cannot mint a content search generation; canonical generation rebuild required"
                .to_owned(),
        )
    }
}

impl ResidentOverlaySnapshot {
    pub(super) fn generation_digest(&self) -> &str {
        &self.state.generation_digest
    }

    pub(super) fn owner_identity_root_digest(&self) -> &str {
        self.state.owner_identity_tree.root_digest().as_str()
    }

    pub(super) fn indexed_owner_paths(&self, base: &WorkspaceMemoryGeneration) -> Vec<String> {
        let mut paths = base
            .owners
            .iter()
            .map(|owner| owner.owner_path.clone())
            .chain(self.state.owners.keys().cloned())
            .collect::<BTreeSet<_>>();
        paths.retain(|path| !self.state.tombstones.contains(path));
        for (owner_path, value) in self.state.content_owners.entries() {
            match value {
                OwnerContentOverlayValue::Present { .. } => {
                    paths.insert(owner_path);
                }
                OwnerContentOverlayValue::Removed => {
                    paths.remove(&owner_path);
                }
            }
        }
        paths.into_iter().collect()
    }

    pub(super) fn merge_grep_candidates(
        &self,
        base_candidates: Vec<String>,
        plan: &agent_semantic_search::ResidentGrepCandidatePlan,
        authority: Option<&agent_semantic_search::ResidentSearchAuthority>,
        limit: usize,
    ) -> Result<Vec<String>, String> {
        let mut candidates = base_candidates
            .into_iter()
            .filter(|owner_path| {
                !self.state.tombstones.contains(owner_path)
                    && !self.state.owners.contains_key(owner_path)
                    && self.state.content_owners.get(owner_path).is_none()
            })
            .collect::<BTreeSet<_>>();
        for (owner_path, owner) in self.state.owners.iter() {
            if self.state.tombstones.contains(owner_path)
                || authority.is_some_and(|required| owner.authority.as_ref() != Some(required))
            {
                continue;
            }
            let grams = self
                .state
                .owner_grams
                .get(owner_path)
                .ok_or_else(|| "resident owner overlay gram index is missing".to_owned())?;
            if owner_grams_match_plan(grams, plan) {
                candidates.insert(owner_path.clone());
            }
        }
        for (owner_path, value) in self.state.content_owners.entries() {
            let OwnerContentOverlayValue::Present { owner, grams } = value else {
                continue;
            };
            if authority.is_some_and(|required| owner.authority.as_ref() != Some(required)) {
                continue;
            }
            if owner_grams_match_plan(grams, plan) {
                candidates.insert(owner_path);
            }
        }
        if candidates.len() > limit {
            return Err(format!(
                "query-not-ready: resident GREP candidate budget exceeded: candidates={} limit={limit}",
                candidates.len()
            ));
        }
        Ok(candidates.into_iter().collect())
    }

    pub(super) fn merge_tantivy_owner_paths(
        &self,
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
        let mut layer = self.state.tantivy_delta_head.as_deref();
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

    pub(super) fn merge_symbol_skeleton_hits(
        &self,
        base_hits: Vec<super::WorkspaceSymbolSkeletonHit>,
        query: &str,
        limit: usize,
    ) -> Vec<super::WorkspaceSymbolSkeletonHit> {
        let mut shadowed = BTreeSet::new();
        let mut seen = BTreeSet::new();
        let mut merged = Vec::new();
        let mut layer = self.state.symbol_delta_head.as_deref();
        while let Some(current) = layer {
            shadowed.extend(current.shadowed_owner_paths.iter().cloned());
            if let Some(index) = &current.index {
                for hit in index.query(query, limit.max(1)) {
                    let key = (hit.owner_path.clone(), hit.structural_selector.clone());
                    if seen.insert(key) {
                        merged.push(hit);
                        if merged.len() == limit {
                            return merged;
                        }
                    }
                }
            }
            layer = current.previous.as_deref();
        }
        for hit in base_hits {
            let key = (hit.owner_path.clone(), hit.structural_selector.clone());
            if !shadowed.contains(&hit.owner_path) && seen.insert(key) {
                merged.push(hit);
                if merged.len() == limit {
                    break;
                }
            }
        }
        merged
    }

    pub(super) fn semantic_owner_materialized(
        &self,
        base: &super::WorkspaceMemoryBackend,
        owner_path: &str,
    ) -> bool {
        if self.state.semantic_owners.contains(owner_path)
            && self.state.owners.contains_key(owner_path)
        {
            return true;
        }
        if self.state.content_owners.get(owner_path).is_some() {
            // A byte mutation invalidates parser-owned semantics until the
            // provider rebinds this exact owner content identity.
            return false;
        }
        if self.state.tombstones.contains(owner_path) {
            return false;
        }
        if self.state.owners.contains_key(owner_path) {
            return self.state.semantic_owners.contains(owner_path);
        }
        base.owner_snapshot(owner_path)
            .is_some_and(base_owner_is_semantic)
    }

    pub(super) fn topology_source_segments(
        &self,
        base: &WorkspaceMemoryGeneration,
    ) -> Vec<super::WorkspaceTopologySourceSegment> {
        let mut owner_paths = base
            .owners
            .iter()
            .map(|owner| owner.owner_path.clone())
            .chain(self.state.owners.keys().cloned())
            .collect::<BTreeSet<_>>();
        owner_paths.retain(|owner| !self.state.tombstones.contains(owner));
        for (owner_path, value) in self.state.content_owners.entries() {
            match value {
                OwnerContentOverlayValue::Present { .. } => {
                    owner_paths.insert(owner_path);
                }
                OwnerContentOverlayValue::Removed => {
                    owner_paths.remove(&owner_path);
                }
            }
        }
        self.topology_source_segments_for_owner_scope(base, &owner_paths)
    }

    pub(super) fn topology_source_segments_for_owner_scope(
        &self,
        base: &WorkspaceMemoryGeneration,
        owner_paths: &BTreeSet<String>,
    ) -> Vec<super::WorkspaceTopologySourceSegment> {
        owner_paths
            .iter()
            .filter(|owner| !self.state.tombstones.contains(*owner))
            .filter_map(|owner_path| {
                let owner = self.owner_snapshot(base, owner_path)?;
                Some(super::WorkspaceTopologySourceSegment {
                    relations: self.owner_relations(base, owner_path),
                    selectors: owner
                        .selectors
                        .iter()
                        .map(|selector| selector.selector.clone())
                        .collect(),
                    owner_path: owner.owner_path,
                    content_digest: owner.content_digest,
                    authority: owner.authority,
                })
            })
            .collect()
    }

    #[expect(
        clippy::type_complexity,
        reason = "the V1 projection returns its three typed evidence collections"
    )]
    pub(super) fn native_syntax_playbook_projection(
        &self,
        base: &WorkspaceMemoryGeneration,
        owner_paths: &[String],
    ) -> Result<
        (
            Vec<agent_semantic_search::NativeSyntaxProjection>,
            Vec<agent_semantic_search::NativeSyntaxRelation>,
            Vec<agent_semantic_search::NativeSyntaxDiagnostic>,
        ),
        String,
    > {
        let admitted = owner_paths
            .iter()
            .map(String::as_str)
            .collect::<BTreeSet<_>>();
        let mut projections = Vec::with_capacity(admitted.len());
        let mut diagnostics = Vec::new();
        let mut relations = Vec::new();
        for owner_path in admitted {
            let owner = self
                .owner_snapshot(base, owner_path)
                .ok_or_else(|| "native syntax playbook owner is absent".to_owned())?;
            if let Some(diagnostic) = owner.native_syntax_diagnostic {
                diagnostics.push(diagnostic);
                continue;
            }
            if owner.selectors.is_empty() && !self.state.semantic_owners.contains(owner_path) {
                diagnostics.push(agent_semantic_search::NativeSyntaxDiagnostic {
                    owner_path: owner.owner_path,
                    content_digest: owner.content_digest,
                    reason_kind: "source-syntax-unavailable".to_owned(),
                    message: "the admitted owner has no parser-owned selectors".to_owned(),
                });
                continue;
            }
            let selectors = owner
                .selectors
                .iter()
                .map(|selector| {
                    let encoded =
                        serde_json::to_vec(&selector.derived_projections).map_err(|error| {
                            format!("encode resident native syntax projections: {error}")
                        })?;
                    Ok(agent_semantic_search::NativeSyntaxSelector {
                        selector: selector.selector.clone(),
                        byte_start: selector.byte_start,
                        byte_end: selector.byte_end,
                        query_keys: selector.query_keys.clone(),
                        derived_projection_digest: format!(
                            "blake3-256:{}",
                            blake3::hash(&encoded).to_hex()
                        ),
                    })
                })
                .collect::<Result<Vec<_>, String>>()?;
            for relation in self.owner_relations(base, owner_path) {
                let encoded = serde_json::to_vec(&relation.relation)
                    .map_err(|error| format!("encode resident native syntax relation: {error}"))?;
                relations.push(agent_semantic_search::NativeSyntaxRelation {
                    owner_path: owner_path.to_owned(),
                    relation_digest: format!("blake3-256:{}", blake3::hash(&encoded).to_hex()),
                });
            }
            projections.push(agent_semantic_search::NativeSyntaxProjection {
                owner_path: owner.owner_path,
                content_digest: owner.content_digest,
                selectors,
            });
        }
        projections.sort_by(|left, right| left.owner_path.cmp(&right.owner_path));
        diagnostics.sort_by(|left, right| left.owner_path.cmp(&right.owner_path));
        relations.sort_by(|left, right| {
            left.owner_path
                .cmp(&right.owner_path)
                .then_with(|| left.relation_digest.cmp(&right.relation_digest))
        });
        Ok((projections, relations, diagnostics))
    }

    pub(super) fn owner_bytes(
        &self,
        base: &WorkspaceMemoryGeneration,
        owner_path: &str,
    ) -> Option<Arc<[u8]>> {
        self.owner_snapshot(base, owner_path)
            .map(|owner| Arc::from(owner.bytes))
    }

    pub(super) fn owner_snapshot(
        &self,
        base: &WorkspaceMemoryGeneration,
        owner_path: &str,
    ) -> Option<WorkspaceOwnerSnapshot> {
        self.state.owner_snapshot(base, owner_path)
    }

    pub(super) fn owner_snapshot_indexed(
        &self,
        base: &super::WorkspaceMemoryBackend,
        owner_path: &str,
    ) -> Option<WorkspaceOwnerSnapshot> {
        if self.state.semantic_owners.contains(owner_path)
            && let Some(owner) = self.state.owners.get(owner_path)
        {
            return Some(owner.clone());
        }
        if let Some(content) = self.state.content_owners.get(owner_path) {
            return match content {
                OwnerContentOverlayValue::Present { owner, .. } => Some(owner.as_ref().clone()),
                OwnerContentOverlayValue::Removed => None,
            };
        }
        if self.state.tombstones.contains(owner_path) {
            return None;
        }
        self.state
            .owners
            .get(owner_path)
            .cloned()
            .or_else(|| base.owner_snapshot(owner_path).cloned())
    }

    fn owner_relations(
        &self,
        base: &WorkspaceMemoryGeneration,
        owner_path: &str,
    ) -> Vec<crate::ClientDbSourceIndexOwnedRelation> {
        if self.state.semantic_owners.contains(owner_path)
            && let Some(relations) = self.state.relations.get(owner_path)
        {
            return relations.clone();
        }
        if self.state.content_owners.get(owner_path).is_some() {
            return Vec::new();
        }
        if self.state.tombstones.contains(owner_path) {
            return Vec::new();
        }
        if let Some(relations) = self.state.relations.get(owner_path) {
            return relations.clone();
        }
        if self.state.owners.contains_key(owner_path) {
            return Vec::new();
        }
        base.relations
            .iter()
            .filter(|relation| relation.owner_path.as_str() == owner_path)
            .cloned()
            .collect()
    }

    pub(super) fn read_selector(
        &self,
        base: &super::WorkspaceMemoryBackend,
        projection_kind: super::model::ExactProjectionKind,
        structural_selector: &str,
    ) -> Result<WorkspaceRuntimeSelectorRead, String> {
        let owner_path = selector_owner_path(structural_selector)?;
        if !self.state.semantic_owners.contains(&owner_path)
            && let Some(content) = self.state.content_owners.get(&owner_path)
        {
            return match content {
                OwnerContentOverlayValue::Present { owner, .. } => read_owner_selector(
                    owner,
                    &self.state.generation_digest,
                    self.state.owner_identity_tree.root_digest().as_str(),
                    projection_kind,
                    structural_selector,
                ),
                OwnerContentOverlayValue::Removed => {
                    Ok(WorkspaceRuntimeSelectorRead::OwnerMissing {
                        generation_digest: self.state.generation_digest.clone(),
                        root_digest: self
                            .state
                            .owner_identity_tree
                            .root_digest()
                            .as_str()
                            .to_owned(),
                    })
                }
            };
        }
        if let Some(overlay) = self
            .state
            .selectors
            .get(&(projection_kind, structural_selector.to_owned()))
        {
            return Ok(WorkspaceRuntimeSelectorRead::Projection {
                generation_digest: self.state.generation_digest.clone(),
                root_digest: self.state.workspace_snapshot.root_digest().to_owned(),
                resolved_selector: structural_selector.to_owned(),
                bytes: overlay.projection_bytes.clone(),
            });
        }
        if self.state.tombstones.contains(&owner_path) {
            return Ok(WorkspaceRuntimeSelectorRead::OwnerMissing {
                generation_digest: self.state.generation_digest.clone(),
                root_digest: self.state.workspace_snapshot.root_digest().to_owned(),
            });
        }
        if let Some(owner) = self.state.owners.get(&owner_path) {
            return read_owner_selector(
                owner,
                &self.state.generation_digest,
                self.state.workspace_snapshot.root_digest(),
                projection_kind,
                structural_selector,
            );
        }
        read_indexed_base_selector_with_identity(
            base,
            &self.state.generation_digest,
            self.state.workspace_snapshot.root_digest(),
            projection_kind,
            structural_selector,
        )
    }
}

impl ResidentOverlayState {
    fn empty() -> Self {
        Self {
            base_generation_digest: String::new(),
            generation_digest: String::new(),
            revision: 0,
            workspace_snapshot: Arc::new(
                agent_semantic_content_identity::WorkspaceSnapshot::from_file_hashes(
                    std::iter::empty::<(String, String)>(),
                ),
            ),
            owner_identity_tree: Arc::new(
                agent_semantic_content_identity::workspace_merkle_v1::WorkspacePathMerkleTreeIncrementalV1::empty(),
            ),
            content_owners: OwnerContentOverlayTrie::default(),
            tantivy_delta_head: None,
            symbol_delta_head: None,
            owners: Arc::new(HashMap::new()),
            relations: Arc::new(HashMap::new()),
            tombstones: Arc::new(HashSet::new()),
            semantic_owners: Arc::new(HashSet::new()),
            owner_grams: Arc::new(HashMap::new()),
            selectors: Arc::new(HashMap::new()),
        }
    }

    fn new(generation: &WorkspaceMemoryGeneration) -> Self {
        let owner_identity_tree =
            agent_semantic_content_identity::workspace_merkle_v1::WorkspacePathMerkleTreeIncrementalV1::from_file_digests(
                generation.owners.iter().map(|owner| {
                    (
                        owner.owner_path.clone(),
                        parse_typed_content_digest(&owner.content_digest)
                            .expect("validated generation owner content digest"),
                    )
                }),
            )
            .expect("validated generation owner paths");
        Self {
            base_generation_digest: generation.generation_digest.clone(),
            generation_digest: generation.generation_digest.clone(),
            revision: 0,
            // The canonical generation is already immutable and reachable
            // through the lease. Keeping a second copy here turns the first
            // candidate parser delta into O(workspace source bytes). This
            // state owns only changed owners; reads fall through to `base`.
            workspace_snapshot: Arc::new(generation.workspace_snapshot.clone()),
            owner_identity_tree: Arc::new(owner_identity_tree),
            content_owners: OwnerContentOverlayTrie::default(),
            tantivy_delta_head: None,
            symbol_delta_head: None,
            owners: Arc::new(HashMap::new()),
            relations: Arc::new(HashMap::new()),
            tombstones: Arc::new(HashSet::new()),
            semantic_owners: Arc::new(HashSet::new()),
            owner_grams: Arc::new(HashMap::new()),
            selectors: Arc::new(HashMap::new()),
        }
    }

    fn advance(&mut self) {
        self.revision = self.revision.saturating_add(1);
        let mut hasher = blake3::Hasher::new();
        hasher.update(self.base_generation_digest.as_bytes());
        hasher.update(&self.revision.to_le_bytes());
        let mut owner_digests = self
            .owners
            .iter()
            .map(|(path, owner)| (path, &owner.content_digest))
            .collect::<Vec<_>>();
        owner_digests.sort_unstable();
        for (path, digest) in owner_digests {
            hasher.update(path.as_bytes());
            hasher.update(digest.as_bytes());
        }
        let mut tombstones = self.tombstones.iter().collect::<Vec<_>>();
        tombstones.sort_unstable();
        for path in tombstones {
            hasher.update(path.as_bytes());
            hasher.update(b"\0removed");
        }
        let mut semantic_owners = self.semantic_owners.iter().collect::<Vec<_>>();
        semantic_owners.sort_unstable();
        for path in semantic_owners {
            hasher.update(path.as_bytes());
            hasher.update(b"\0semantic");
        }
        self.generation_digest = format!("blake3-256:{}", hasher.finalize().to_hex());
    }

    fn advance_owner_content_mutation(
        &mut self,
        operations: &[agent_semantic_content_identity::workspace_merkle_v1::WorkspaceMerkleDeltaOperationIncrementalV1],
    ) {
        self.revision = self.revision.saturating_add(1);
        let mut hasher = blake3::Hasher::new();
        hasher.update(b"agent.semantic-protocols.owner-content-transaction.v1\0");
        hasher.update(self.generation_digest.as_bytes());
        hasher.update(&self.revision.to_le_bytes());
        for operation in operations {
            match operation {
                agent_semantic_content_identity::workspace_merkle_v1::WorkspaceMerkleDeltaOperationIncrementalV1::Upsert {
                    owner_path,
                    source_blob_digest,
                    ..
                } => {
                    hasher.update(b"upsert\0");
                    hasher.update(owner_path.as_bytes());
                    hasher.update(source_blob_digest.as_str().as_bytes());
                }
                agent_semantic_content_identity::workspace_merkle_v1::WorkspaceMerkleDeltaOperationIncrementalV1::Remove {
                    owner_path,
                    previous_source_blob_digest,
                } => {
                    hasher.update(b"remove\0");
                    hasher.update(owner_path.as_bytes());
                    hasher.update(previous_source_blob_digest.as_str().as_bytes());
                }
            }
        }
        hasher.update(self.owner_identity_tree.root_digest().as_str().as_bytes());
        self.generation_digest = format!("blake3-256:{}", hasher.finalize().to_hex());
    }
}

impl ResidentOverlayState {
    fn owner_snapshot(
        &self,
        base: &WorkspaceMemoryGeneration,
        owner_path: &str,
    ) -> Option<WorkspaceOwnerSnapshot> {
        if self.semantic_owners.contains(owner_path)
            && let Some(owner) = self.owners.get(owner_path)
        {
            return Some(owner.clone());
        }
        if let Some(content) = self.content_owners.get(owner_path) {
            return match content {
                OwnerContentOverlayValue::Present { owner, .. } => Some(owner.as_ref().clone()),
                OwnerContentOverlayValue::Removed => None,
            };
        }
        if self.tombstones.contains(owner_path) {
            return None;
        }
        self.owners
            .get(owner_path)
            .cloned()
            .or_else(|| base_owner(base, owner_path).cloned())
    }
}

fn parse_typed_content_digest(
    digest: &str,
) -> Result<agent_semantic_content_identity::exact_selector_merkle::ContentDigestV1, String> {
    let payload = digest
        .strip_prefix("blake3-256:")
        .ok_or_else(|| "owner content digest must use blake3-256".to_owned())?;
    agent_semantic_content_identity::exact_selector_merkle::parse_content_digest_v1(payload)
}

fn owner_byte_grams(bytes: &[u8]) -> BTreeSet<u32> {
    bytes
        .windows(agent_semantic_search::RESIDENT_BYTE_GRAM_WIDTH)
        .map(|window| {
            (u32::from(window[0]) << 16) | (u32::from(window[1]) << 8) | u32::from(window[2])
        })
        .collect()
}

fn workspace_owner_symbol_skeleton(
    owner: &WorkspaceOwnerSnapshot,
) -> agent_semantic_symbol_index::SymbolSkeletonOwnerV1 {
    agent_semantic_symbol_index::SymbolSkeletonOwnerV1 {
        owner_path: owner.owner_path.clone(),
        owner_content_digest: owner.content_digest.clone(),
        language_id: owner
            .authority
            .as_ref()
            .map(|authority| authority.language_id.as_str().to_owned()),
        symbols: owner
            .selectors
            .iter()
            .filter(|selector| !selector.query_keys.is_empty())
            .map(
                |selector| agent_semantic_symbol_index::SymbolSkeletonRecordV1 {
                    structural_selector: selector.selector.clone(),
                    keys: selector.query_keys.clone(),
                },
            )
            .collect(),
    }
}

fn workspace_owner_symbol_document(
    owner: &WorkspaceOwnerSnapshot,
) -> agent_semantic_search::ResidentSourceDocument {
    agent_semantic_search::ResidentSourceDocument {
        owner_path: owner.owner_path.clone(),
        owner_content_digest: owner.content_digest.clone(),
        line_count: u32::try_from(owner.bytes.iter().filter(|byte| **byte == b'\n').count() + 1)
            .unwrap_or(u32::MAX),
        query_keys: agent_semantic_search::resident_skeleton_coverage_keys(
            &owner.owner_path,
            owner
                .selectors
                .iter()
                .flat_map(|selector| selector.query_keys.iter().cloned()),
        ),
        authority: owner.authority.clone(),
    }
}

fn owner_grams_match_plan(
    grams: &BTreeSet<u32>,
    plan: &agent_semantic_search::ResidentGrepCandidatePlan,
) -> bool {
    match plan {
        agent_semantic_search::ResidentGrepCandidatePlan::MatchAll => true,
        agent_semantic_search::ResidentGrepCandidatePlan::Grams(required) => {
            required.iter().all(|gram| grams.contains(gram))
        }
        agent_semantic_search::ResidentGrepCandidatePlan::And(plans) => {
            plans.iter().all(|plan| owner_grams_match_plan(grams, plan))
        }
        agent_semantic_search::ResidentGrepCandidatePlan::Or(plans) => {
            plans.iter().any(|plan| owner_grams_match_plan(grams, plan))
        }
    }
}

fn selector_owner_path(selector: &str) -> Result<String, String> {
    agent_semantic_content_identity::CanonicalItemSelector::parse_root_or_exact_descendant(selector)
        .map_err(|error| format!("exact structural selector is not canonical: {error}"))?
        .owner_path()
}

fn base_owner<'a>(
    base: &'a WorkspaceMemoryGeneration,
    owner_path: &str,
) -> Option<&'a WorkspaceOwnerSnapshot> {
    base.owners
        .iter()
        .find(|owner| owner.owner_path == owner_path)
}

fn base_owner_is_semantic(owner: &WorkspaceOwnerSnapshot) -> bool {
    !owner.selectors.is_empty() || owner.native_syntax_diagnostic.is_some()
}

fn validate_owner(owner: &WorkspaceOwnerSnapshot) -> Result<(), String> {
    let digest = format!(
        "blake3-256:{}",
        agent_semantic_content_identity::ArtifactHash::blake3(&owner.bytes).value
    );
    if owner.content_digest != digest {
        return Err("runtime owner overlay content digest drift".to_owned());
    }
    if let Some(diagnostic) = &owner.native_syntax_diagnostic
        && (diagnostic.owner_path != owner.owner_path
            || diagnostic.content_digest != owner.content_digest
            || diagnostic.reason_kind != "source-syntax-unavailable"
            || diagnostic.message.trim().is_empty()
            || !owner.selectors.is_empty())
    {
        return Err("runtime owner overlay native syntax diagnostic drift".to_owned());
    }
    Ok(())
}

fn validate_selector_overlay(
    owner: &WorkspaceOwnerSnapshot,
    capability: &crate::active_generation_projection_capability::ActiveGenerationProjectionCapabilityManifest,
    overlay: &WorkspaceRuntimeSelectorOverlay,
) -> Result<(), String> {
    if owner.content_digest != overlay.owner_content_digest {
        return Err("runtime selector overlay owner digest drift".to_owned());
    }
    if let Some(selector) = owner
        .selectors
        .iter()
        .find(|selector| selector.selector == overlay.structural_selector)
    {
        if selector.byte_start != overlay.byte_start || selector.byte_end != overlay.byte_end {
            return Err("runtime selector overlay range drifts from the admitted owner".to_owned());
        }
    } else {
        let projection_mode = match overlay.projection_kind {
            super::model::ExactProjectionKind::Source => crate::active_generation_projection_capability::ActiveGenerationProjectionMode::Source,
            super::model::ExactProjectionKind::CallableSkeleton => crate::active_generation_projection_capability::ActiveGenerationProjectionMode::CallableSkeleton,
        };
        let admitted = capability.selectors.iter().any(|selector| {
            selector.owner_path == overlay.owner_path
                && selector.selector == overlay.structural_selector
                && selector.projection_modes.contains(&projection_mode)
        });
        if !admitted {
            return Err(
                "runtime selector overlay is not declared by the active generation capability"
                    .to_owned(),
            );
        }
    }
    let source = owner
        .bytes
        .get(overlay.byte_start..overlay.byte_end)
        .ok_or_else(|| "runtime selector overlay range is invalid".to_owned())?;
    if overlay.projection_kind == super::model::ExactProjectionKind::Source
        && source != overlay.projection_bytes
    {
        return Err(
            "runtime source selector overlay bytes do not match the admitted owner range"
                .to_owned(),
        );
    }
    Ok(())
}

fn read_indexed_base_selector_with_identity(
    base: &super::WorkspaceMemoryBackend,
    generation_digest: &str,
    root_digest: &str,
    projection_kind: super::model::ExactProjectionKind,
    structural_selector: &str,
) -> Result<WorkspaceRuntimeSelectorRead, String> {
    if let Some((owner, selector)) = base.selector_snapshot(structural_selector) {
        return read_resolved_selector(
            owner,
            selector,
            generation_digest,
            root_digest,
            projection_kind,
            structural_selector,
        );
    }
    let owner_path = selector_owner_path(structural_selector)?;
    if let Some(owner) = base.owner_snapshot(&owner_path) {
        return read_owner_without_resolved_selector(
            owner,
            generation_digest,
            root_digest,
            projection_kind,
            structural_selector,
        );
    }
    Ok(WorkspaceRuntimeSelectorRead::OwnerMissing {
        generation_digest: generation_digest.to_owned(),
        root_digest: root_digest.to_owned(),
    })
}

fn read_owner_selector(
    owner: &WorkspaceOwnerSnapshot,
    generation_digest: &str,
    root_digest: &str,
    projection_kind: super::model::ExactProjectionKind,
    structural_selector: &str,
) -> Result<WorkspaceRuntimeSelectorRead, String> {
    if let Some(selector) = owner
        .selectors
        .iter()
        .find(|selector| selector.selector == structural_selector)
    {
        return read_resolved_selector(
            owner,
            selector,
            generation_digest,
            root_digest,
            projection_kind,
            structural_selector,
        );
    }
    read_owner_without_resolved_selector(
        owner,
        generation_digest,
        root_digest,
        projection_kind,
        structural_selector,
    )
}

fn read_resolved_selector(
    owner: &WorkspaceOwnerSnapshot,
    selector: &super::WorkspaceSelectorSnapshot,
    generation_digest: &str,
    root_digest: &str,
    projection_kind: super::model::ExactProjectionKind,
    structural_selector: &str,
) -> Result<WorkspaceRuntimeSelectorRead, String> {
    if projection_kind == super::model::ExactProjectionKind::Source {
        let bytes = owner
            .bytes
            .get(selector.byte_start..selector.byte_end)
            .ok_or_else(|| "resident selector range is invalid".to_owned())?
            .to_vec();
        return Ok(WorkspaceRuntimeSelectorRead::Projection {
            generation_digest: generation_digest.to_owned(),
            root_digest: root_digest.to_owned(),
            resolved_selector: structural_selector.to_owned(),
            bytes,
        });
    }
    if let Some(derived) = selector
        .derived_projections
        .iter()
        .find(|derived| derived.projection_kind == projection_kind)
    {
        return Ok(WorkspaceRuntimeSelectorRead::Projection {
            generation_digest: generation_digest.to_owned(),
            root_digest: root_digest.to_owned(),
            resolved_selector: structural_selector.to_owned(),
            bytes: derived.bytes.clone(),
        });
    }
    read_owner_without_resolved_selector(
        owner,
        generation_digest,
        root_digest,
        projection_kind,
        structural_selector,
    )
}

fn read_owner_without_resolved_selector(
    owner: &WorkspaceOwnerSnapshot,
    generation_digest: &str,
    root_digest: &str,
    projection_kind: super::model::ExactProjectionKind,
    _structural_selector: &str,
) -> Result<WorkspaceRuntimeSelectorRead, String> {
    // Source is an owner-level projection when this admitted owner has no
    // parser selectors.  Derived projections still fail closed below: they
    // need parser materialization rather than raw source bytes.
    if projection_kind == super::model::ExactProjectionKind::Source && owner.selectors.is_empty() {
        return Ok(WorkspaceRuntimeSelectorRead::Projection {
            generation_digest: generation_digest.to_owned(),
            root_digest: root_digest.to_owned(),
            resolved_selector: owner.owner_path.clone(),
            bytes: owner.bytes.clone(),
        });
    }
    Ok(WorkspaceRuntimeSelectorRead::OwnerForRepair {
        generation_digest: generation_digest.to_owned(),
        root_digest: root_digest.to_owned(),
        owner: owner.clone(),
    })
}
