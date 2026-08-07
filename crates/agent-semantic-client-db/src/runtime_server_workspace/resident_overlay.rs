//! Millisecond resident owner and selector overlays behind one writer lane.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use parking_lot::RwLock;

use super::{
    WorkspaceDerivedProjectionSnapshot, WorkspaceMemoryGeneration, WorkspaceOwnerSnapshot,
    WorkspaceRuntimeSelectorOverlay, WorkspaceRuntimeSelectorOverlayReceipt,
    WorkspaceRuntimeSelectorRead, WorkspaceSelectorSnapshot,
};

#[derive(Debug)]
pub(crate) struct ResidentOverlayStore {
    state: RwLock<ResidentOverlayState>,
}

#[derive(Debug, Clone)]
struct ResidentOverlayState {
    base_generation_digest: String,
    generation_digest: String,
    base_epoch: u64,
    revision: u64,
    workspace_snapshot: agent_semantic_content_identity::WorkspaceSnapshot,
    owners: HashMap<String, WorkspaceOwnerSnapshot>,
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
        validate_selector_overlay(owner, &overlay)?;
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
        base: &WorkspaceMemoryGeneration,
    ) -> Result<WorkspaceMemoryGeneration, String> {
        let mut owners = base
            .owners
            .iter()
            .filter(|owner| !self.state.tombstones.contains(&owner.owner_path))
            .map(|owner| (owner.owner_path.clone(), owner.clone()))
            .collect::<std::collections::BTreeMap<_, _>>();
        for (owner_path, owner) in &self.state.owners {
            owners.insert(owner_path.clone(), owner.clone());
        }
        for overlay in self.state.selectors.values() {
            let owner = owners.get_mut(&overlay.owner_path).ok_or_else(|| {
                format!(
                    "runtime selector overlay omitted its owner during generation materialization: ownerPath={}",
                    overlay.owner_path
                )
            })?;
            materialize_selector(owner, overlay)?;
        }
        let mut owners = owners.into_values().collect::<Vec<_>>();
        for owner in &mut owners {
            owner
                .selectors
                .sort_by(|left, right| left.selector.cmp(&right.selector));
        }
        let mut source_snapshot = self.state.workspace_snapshot.evidence(
            agent_semantic_content_identity::SourceSnapshotKind::DerivedOverlay,
            base.source_snapshot.provider_digest.clone(),
        );
        source_snapshot.base_root_digest = Some(base.source_snapshot.root_digest.clone());
        source_snapshot.dirty_paths_digest = Some(overlay_delta_digest(&self.state));
        WorkspaceMemoryGeneration::try_from_build(super::model::WorkspaceGenerationBuild {
            workspace_identity: base.workspace_identity.clone(),
            project_root: base.project_root.clone(),
            active_epoch: self.epoch(),
            workspace_snapshot: self.state.workspace_snapshot.clone(),
            source_snapshot,
            module_graph_digest: base.module_graph_digest.clone(),
            project_resolutions: base.project_resolutions.clone(),
            relations: base.relations.clone(),
            owners,
        })
    }
}

fn materialize_selector(
    owner: &mut WorkspaceOwnerSnapshot,
    overlay: &WorkspaceRuntimeSelectorOverlay,
) -> Result<(), String> {
    validate_selector_overlay(owner, overlay)?;
    let selector = match owner
        .selectors
        .iter_mut()
        .find(|selector| selector.selector == overlay.structural_selector)
    {
        Some(selector) => selector,
        None => {
            owner.selectors.push(WorkspaceSelectorSnapshot {
                selector: overlay.structural_selector.clone(),
                byte_start: overlay.byte_start,
                byte_end: overlay.byte_end,
                derived_projections: Vec::new(),
            });
            owner.selectors.last_mut().expect("selector was inserted")
        }
    };
    selector.byte_start = overlay.byte_start;
    selector.byte_end = overlay.byte_end;
    if overlay.projection_kind != super::model::ExactProjectionKind::Source {
        if let Some(projection) = selector
            .derived_projections
            .iter_mut()
            .find(|projection| projection.projection_kind == overlay.projection_kind)
        {
            projection.bytes.clone_from(&overlay.projection_bytes);
        } else {
            selector
                .derived_projections
                .push(WorkspaceDerivedProjectionSnapshot {
                    projection_kind: overlay.projection_kind,
                    bytes: overlay.projection_bytes.clone(),
                });
        }
        selector
            .derived_projections
            .sort_by(|left, right| left.projection_kind.cmp(&right.projection_kind));
    }
    Ok(())
}

fn overlay_delta_digest(state: &ResidentOverlayState) -> String {
    let mut paths = state
        .owners
        .keys()
        .chain(state.tombstones.iter())
        .cloned()
        .collect::<Vec<_>>();
    paths.sort();
    paths.dedup();
    let mut hasher = blake3::Hasher::new();
    for path in paths {
        hasher.update(path.as_bytes());
        hasher.update(b"\0");
    }
    format!("blake3-256:{}", hasher.finalize().to_hex())
}

impl ResidentOverlaySnapshot {
    pub(super) fn generation_digest(&self) -> &str {
        &self.state.generation_digest
    }

    pub(super) fn epoch(&self) -> u64 {
        self.state.base_epoch.saturating_add(self.state.revision)
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
        if self.state.tombstones.contains(owner_path) {
            return Ok(WorkspaceRuntimeSelectorRead::OwnerMissing {
                generation_digest: self.state.generation_digest.clone(),
                root_digest: self.state.workspace_snapshot.root_digest().to_owned(),
            });
        }
        if let Some(owner) = self.state.owners.get(owner_path) {
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
            base_epoch: 0,
            revision: 0,
            workspace_snapshot:
                agent_semantic_content_identity::WorkspaceSnapshot::from_file_hashes(
                    std::iter::empty::<(String, String)>(),
                ),
            owners: HashMap::new(),
            tombstones: HashSet::new(),
            selectors: HashMap::new(),
        }
    }

    fn new(generation: &WorkspaceMemoryGeneration) -> Self {
        Self {
            base_generation_digest: generation.generation_digest.clone(),
            generation_digest: generation.generation_digest.clone(),
            base_epoch: generation.active_epoch,
            revision: 0,
            workspace_snapshot: generation.workspace_snapshot.clone(),
            owners: HashMap::new(),
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

fn selector_owner_path(selector: &str) -> Result<&str, String> {
    selector
        .split_once("://")
        .and_then(|(_, selector)| selector.split_once('#'))
        .map(|(owner_path, _)| owner_path)
        .ok_or_else(|| "exact structural selector is missing its owner path".to_owned())
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
    Ok(())
}

fn validate_selector_overlay(
    owner: &WorkspaceOwnerSnapshot,
    overlay: &WorkspaceRuntimeSelectorOverlay,
) -> Result<(), String> {
    if owner.content_digest != overlay.owner_content_digest {
        return Err("runtime selector overlay owner digest drift".to_owned());
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
    if let Some(owner) = base_owner(base, owner_path) {
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
    Ok(WorkspaceRuntimeSelectorRead::OwnerForRepair {
        generation_digest: generation_digest.to_owned(),
        root_digest: root_digest.to_owned(),
        owner: owner.clone(),
    })
}
