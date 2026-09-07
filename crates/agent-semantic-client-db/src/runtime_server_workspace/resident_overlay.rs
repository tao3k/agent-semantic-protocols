// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Millisecond resident owner and selector overlays behind one writer lane.

use std::collections::{HashMap, HashSet};
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
    workspace_snapshot: agent_semantic_content_identity::WorkspaceSnapshot,
    owners: HashMap<String, WorkspaceOwnerSnapshot>,
    relations: HashMap<String, Vec<crate::ClientDbSourceIndexOwnedRelation>>,
    tombstones: HashSet<String>,
    selectors:
        HashMap<(super::model::ExactProjectionKind, String), WorkspaceRuntimeSelectorOverlay>,
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
        state.owners.insert(owner_path.clone(), owner);
        state.relations.insert(owner_path.clone(), Vec::new());
        state.tombstones.remove(&owner_path);
        state
            .selectors
            .retain(|_, selector| selector.owner_path != owner_path);
        state.workspace_snapshot = state
            .workspace_snapshot
            .with_overlay([(owner_path, owner_digest)]);
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
            state.owners.insert(owner_path.clone(), owner);
            state.relations.insert(owner_path.clone(), Vec::new());
            state.tombstones.remove(&owner_path);
            state
                .selectors
                .retain(|_, selector| selector.owner_path != owner_path);
        }
        for owner_path in tombstones {
            if changed.contains_key(&owner_path) || !removed.insert(owner_path.clone()) {
                return Err(format!(
                    "runtime owner delta contains a duplicate owner: {owner_path}"
                ));
            }
            if state.owners.remove(&owner_path).is_none() && base_owner(base, &owner_path).is_none()
            {
                return Err(format!(
                    "runtime owner tombstone target is unavailable: {owner_path}"
                ));
            }
            state.tombstones.insert(owner_path.clone());
            state.relations.remove(&owner_path);
            state
                .selectors
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
            state
                .relations
                .get_mut(owner_path)
                .expect("changed owner relation bucket was initialized")
                .push(relation);
        }
        state.workspace_snapshot = state
            .workspace_snapshot
            .with_overlay_delta(changed, removed);
        state.advance();
        Ok(ResidentOverlaySnapshot { state })
    }

    pub(super) fn tombstone_owner(
        &self,
        base: &WorkspaceMemoryGeneration,
        owner_path: &str,
    ) -> Result<ResidentOverlaySnapshot, String> {
        let mut state = self.staged_state(base);
        if state.owners.remove(owner_path).is_none() && base_owner(base, owner_path).is_none() {
            return Err("runtime owner tombstone target is unavailable".to_owned());
        }
        state.tombstones.insert(owner_path.to_owned());
        state.relations.remove(owner_path);
        state
            .selectors
            .retain(|_, selector| selector.owner_path != owner_path);
        state.workspace_snapshot = state.workspace_snapshot.with_overlay_delta(
            std::iter::empty::<(String, String)>(),
            [owner_path.to_owned()],
        );
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
        if state.owners.remove(previous_owner_path).is_none()
            && base_owner(base, previous_owner_path).is_none()
        {
            return Err("runtime owner relocation source is unavailable".to_owned());
        }
        state.tombstones.insert(previous_owner_path.to_owned());
        state
            .selectors
            .retain(|_, selector| selector.owner_path != previous_owner_path);
        let owner_path = owner.owner_path.clone();
        let owner_digest = owner.content_digest.clone();
        state.owners.insert(owner_path.clone(), owner);
        state.tombstones.remove(&owner_path);
        state.workspace_snapshot = state.workspace_snapshot.with_overlay_delta(
            [(owner_path, owner_digest)],
            [previous_owner_path.to_owned()],
        );
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
        let key = (
            overlay.projection_kind.clone(),
            overlay.structural_selector.clone(),
        );
        let inserted = match state.selectors.get(&key) {
            Some(existing) if existing == &overlay => false,
            Some(_) => {
                return Err(
                    "runtime selector overlay conflicts with an admitted projection".to_owned(),
                );
            }
            None => {
                state.selectors.insert(key, overlay.clone());
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
        state.owners.insert(owner_path.clone(), owner.clone());
        state.tombstones.remove(&owner_path);
        state
            .selectors
            .retain(|_, selector| selector.owner_path != owner_path);
        state.workspace_snapshot = state
            .workspace_snapshot
            .with_overlay([(owner_path, owner_digest)]);
        validate_selector_overlay(&owner, &base.projection_capability, &overlay)?;
        let key = (
            overlay.projection_kind.clone(),
            overlay.structural_selector.clone(),
        );
        state.selectors.insert(key, overlay.clone());
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
        if self.state.tombstones.contains(owner_path) {
            return None;
        }
        self.state
            .owners
            .get(owner_path)
            .cloned()
            .or_else(|| base_owner(base, owner_path).cloned())
    }

    pub(super) fn read_selector(
        &self,
        base: &WorkspaceMemoryGeneration,
        projection_kind: super::model::ExactProjectionKind,
        structural_selector: &str,
    ) -> Result<WorkspaceRuntimeSelectorRead, String> {
        let owner_path = selector_owner_path(structural_selector)?;
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
        read_base_selector_with_identity(
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
            workspace_snapshot:
                agent_semantic_content_identity::WorkspaceSnapshot::from_file_hashes(
                    std::iter::empty::<(String, String)>(),
                ),
            owners: HashMap::new(),
            relations: HashMap::new(),
            tombstones: HashSet::new(),
            selectors: HashMap::new(),
        }
    }

    fn new(generation: &WorkspaceMemoryGeneration) -> Self {
        Self {
            base_generation_digest: generation.generation_digest.clone(),
            generation_digest: generation.generation_digest.clone(),
            revision: 0,
            workspace_snapshot: generation.workspace_snapshot.clone(),
            owners: generation
                .owners
                .iter()
                .cloned()
                .map(|owner| (owner.owner_path.clone(), owner))
                .collect(),
            relations: generation.relations.iter().cloned().fold(
                HashMap::<String, Vec<crate::ClientDbSourceIndexOwnedRelation>>::new(),
                |mut relations, relation| {
                    relations
                        .entry(relation.owner_path.as_str().to_owned())
                        .or_default()
                        .push(relation);
                    relations
                },
            ),
            tombstones: HashSet::new(),
            selectors: HashMap::new(),
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
        self.generation_digest = format!("blake3-256:{}", hasher.finalize().to_hex());
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

fn read_base_selector_with_identity(
    base: &WorkspaceMemoryGeneration,
    generation_digest: &str,
    root_digest: &str,
    projection_kind: super::model::ExactProjectionKind,
    structural_selector: &str,
) -> Result<WorkspaceRuntimeSelectorRead, String> {
    let owner_path = selector_owner_path(structural_selector)?;
    if let Some(owner) = base_owner(base, &owner_path) {
        return read_owner_selector(
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
    }
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
