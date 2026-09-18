// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Millisecond resident owner and selector overlays behind one writer lane.

use std::collections::{BTreeSet, HashSet};
use std::sync::Arc;

use parking_lot::RwLock;

use super::resident_content_overlay::OwnerContentOverlayValue;
use super::resident_grep_overlay::owner_byte_grams;
use super::resident_overlay_state::{ResidentOverlayState, base_owner, parse_typed_content_digest};
use super::resident_tantivy_overlay::ResidentTantivyDeltaLayer;
use super::resident_topology_overlay::ResidentTopologyDeltaLayer;
use super::{
    WorkspaceMemoryGeneration, WorkspaceOwnerSnapshot, WorkspaceRuntimeSelectorOverlay,
    WorkspaceRuntimeSelectorOverlayReceipt, WorkspaceRuntimeSelectorRead,
};

pub(crate) use super::resident_content_overlay::PreparedOwnerContentSearchDelta;
pub(crate) use super::resident_content_overlay::prepare_owner_content_search_delta;
pub(crate) use super::resident_topology_overlay::PreparedOwnerTopologyRebind;
pub(crate) use super::resident_topology_overlay::prepare_owner_topology_rebind;

#[derive(Debug)]
pub(crate) struct ResidentOverlayStore {
    state: RwLock<ResidentOverlayState>,
}

#[derive(Debug, Clone)]
pub(crate) struct ResidentOverlaySnapshot {
    pub(super) state: ResidentOverlayState,
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
        state.topology_delta_head = Some(Arc::new(ResidentTopologyDeltaLayer {
            shadowed_owner_paths,
            index: None,
            previous: state.topology_delta_head.clone(),
        }));
        state.advance_owner_content_mutation(&operations);
        Ok((ResidentOverlaySnapshot { state }, metrics))
    }

    pub(super) fn publish_owner_topology_rebind(
        &self,
        base: &WorkspaceMemoryGeneration,
        rebind: super::WorkspaceOwnerTopologyRebindV1,
        prepared: PreparedOwnerTopologyRebind,
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
        state.topology_delta_head = Some(Arc::new(ResidentTopologyDeltaLayer {
            shadowed_owner_paths: prepared.shadowed_owner_paths,
            index: Some(prepared.topology_index),
            previous: state.topology_delta_head.clone(),
        }));
        // Parser projection changes semantic membership, not source content.
        // Keep the canonical Merkle snapshot shared and advance this transaction once.
        state.advance();
        Ok((
            ResidentOverlaySnapshot { state },
            prepared.topology_node_count,
        ))
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

    pub(super) fn merge_tantivy_owner_paths(
        &self,
        base_owner_paths: Vec<String>,
        expression: &str,
        language_id: &agent_semantic_client_core::LanguageId,
        admitted_owner_paths: Option<&[String]>,
        limit: usize,
    ) -> Result<Vec<String>, String> {
        super::resident_tantivy_overlay::merge_tantivy_owner_paths(
            self.state.tantivy_delta_head.as_deref(),
            base_owner_paths,
            expression,
            language_id,
            admitted_owner_paths,
            limit,
        )
    }

    pub(super) fn merge_topology_hits(
        &self,
        base_hits: Vec<super::WorkspaceTopologyHit>,
        query: &str,
        limit: usize,
    ) -> Vec<super::WorkspaceTopologyHit> {
        super::resident_topology_overlay::merge_topology_hits(
            self.state.topology_delta_head.as_deref(),
            base_hits,
            query,
            limit,
        )
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

    pub(super) fn owner_relations(
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

fn selector_owner_path(selector: &str) -> Result<String, String> {
    agent_semantic_content_identity::CanonicalStructuralSelectorReference::parse(selector)
        .map_err(|error| format!("exact structural selector is not canonical: {error}"))?
        .owner_path()
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
    structural_selector: &str,
) -> Result<WorkspaceRuntimeSelectorRead, String> {
    // Source is an owner-level projection when this admitted owner has no
    // parser selectors.  Derived projections still fail closed below: they
    // need parser materialization rather than raw source bytes.
    let owner_root = agent_semantic_content_identity::CanonicalStructuralSelectorReference::parse(
        structural_selector,
    )
    .is_ok_and(|selector| {
        matches!(
            selector,
            agent_semantic_content_identity::CanonicalStructuralSelectorReference::OwnerRoot(_)
        )
    });
    if projection_kind == super::model::ExactProjectionKind::Source
        && (owner_root || owner.selectors.is_empty())
    {
        return Ok(WorkspaceRuntimeSelectorRead::Projection {
            generation_digest: generation_digest.to_owned(),
            root_digest: root_digest.to_owned(),
            resolved_selector: structural_selector.to_owned(),
            bytes: owner.bytes.clone(),
        });
    }
    Ok(WorkspaceRuntimeSelectorRead::OwnerForRepair {
        generation_digest: generation_digest.to_owned(),
        root_digest: root_digest.to_owned(),
        owner: owner.clone(),
    })
}
