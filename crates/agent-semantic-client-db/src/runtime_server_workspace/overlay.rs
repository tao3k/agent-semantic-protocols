use std::sync::Arc;

use tokio::sync::watch;

use super::model::WorkspaceMemoryBackend;
use super::{WorkspaceGenerationState, WorkspaceMemoryGeneration, WorkspaceOwnerSnapshot};

pub(super) fn prepare_owner_overlay(
    current: &watch::Sender<Option<Arc<WorkspaceMemoryBackend>>>,
    workspace_identity: String,
    owner: WorkspaceOwnerSnapshot,
) -> Result<(u64, WorkspaceMemoryGeneration), String> {
    let active = active_generation(current, &workspace_identity, "overlay")?;
    let owner_path = owner.owner_path.clone();
    let owner_digest = owner.content_digest.clone();
    let mut owners = active.generation().owners.clone();
    upsert_owner(&mut owners, owner);
    let workspace_snapshot = active
        .generation()
        .workspace_snapshot
        .with_overlay([(owner_path, owner_digest)]);
    finish_overlay(active, workspace_identity, owners, workspace_snapshot)
}

pub(super) fn prepare_owner_tombstone(
    current: &watch::Sender<Option<Arc<WorkspaceMemoryBackend>>>,
    workspace_identity: String,
    owner_path: String,
) -> Result<(u64, WorkspaceMemoryGeneration), String> {
    let active = active_generation(current, &workspace_identity, "tombstone")?;
    let mut owners = active.generation().owners.clone();
    remove_owner(&mut owners, &owner_path)?;
    let workspace_snapshot = active
        .generation()
        .workspace_snapshot
        .with_overlay_delta(std::iter::empty::<(String, String)>(), [owner_path]);
    finish_overlay(active, workspace_identity, owners, workspace_snapshot)
}

pub(super) fn prepare_owner_relocation(
    current: &watch::Sender<Option<Arc<WorkspaceMemoryBackend>>>,
    workspace_identity: String,
    previous_owner_path: String,
    owner: WorkspaceOwnerSnapshot,
) -> Result<(u64, WorkspaceMemoryGeneration), String> {
    if previous_owner_path == owner.owner_path {
        return Err("runtime owner relocation requires two distinct owner paths".to_owned());
    }
    let active = active_generation(current, &workspace_identity, "relocation")?;
    let mut owners = active.generation().owners.clone();
    remove_owner(&mut owners, &previous_owner_path)?;
    let next_owner_path = owner.owner_path.clone();
    let next_owner_digest = owner.content_digest.clone();
    upsert_owner(&mut owners, owner);
    let workspace_snapshot = active.generation().workspace_snapshot.with_overlay_delta(
        [(next_owner_path, next_owner_digest)],
        [previous_owner_path],
    );
    finish_overlay(active, workspace_identity, owners, workspace_snapshot)
}

fn active_generation(
    current: &watch::Sender<Option<Arc<WorkspaceMemoryBackend>>>,
    workspace_identity: &str,
    mutation: &str,
) -> Result<Arc<WorkspaceMemoryBackend>, String> {
    let active = current.borrow().clone().ok_or_else(|| {
        format!(
            "runtime owner {mutation} requires an admitted canonical generation: workspaceIdentity={workspace_identity}"
        )
    })?;
    if active.generation().workspace_identity != workspace_identity {
        return Err(format!(
            "runtime owner {mutation} workspace identity mismatch: requested={workspace_identity} active={}",
            active.generation().workspace_identity
        ));
    }
    Ok(active)
}

fn remove_owner(owners: &mut Vec<WorkspaceOwnerSnapshot>, owner_path: &str) -> Result<(), String> {
    let position = owners
        .iter()
        .position(|owner| owner.owner_path == owner_path)
        .ok_or_else(|| {
            format!(
                "runtime owner tombstone is not in the active generation: ownerPath={owner_path}"
            )
        })?;
    owners.remove(position);
    Ok(())
}

fn upsert_owner(owners: &mut Vec<WorkspaceOwnerSnapshot>, owner: WorkspaceOwnerSnapshot) {
    if let Some(position) = owners
        .iter()
        .position(|candidate| candidate.owner_path == owner.owner_path)
    {
        if owners[position].content_digest == owner.content_digest
            && owners[position].bytes == owner.bytes
        {
            for selector in owner.selectors {
                if let Some(selector_position) = owners[position]
                    .selectors
                    .iter()
                    .position(|candidate| candidate.selector == selector.selector)
                {
                    owners[position].selectors[selector_position] = selector;
                } else {
                    owners[position].selectors.push(selector);
                }
            }
            owners[position]
                .selectors
                .sort_by(|left, right| left.selector.cmp(&right.selector));
        } else {
            owners[position] = owner;
        }
    } else {
        owners.push(owner);
    }
    owners.sort_by(|left, right| left.owner_path.cmp(&right.owner_path));
}

fn finish_overlay(
    active: Arc<WorkspaceMemoryBackend>,
    workspace_identity: String,
    owners: Vec<WorkspaceOwnerSnapshot>,
    workspace_snapshot: agent_semantic_content_identity::WorkspaceSnapshot,
) -> Result<(u64, WorkspaceMemoryGeneration), String> {
    let active_epoch = active.generation().active_epoch;
    let source_snapshot = workspace_snapshot.evidence(
        agent_semantic_content_identity::SourceSnapshotKind::DerivedOverlay,
        active.generation().source_snapshot.provider_digest.clone(),
    );
    let workspace_generation =
        agent_semantic_content_identity::workspace_generation_evidence::WorkspaceGenerationEvidenceV1 {
            root_digest: source_snapshot.root_digest.clone(),
            root_depth: u32::from(active.generation().root_depth[0]),
            leaf_count: u64::try_from(source_snapshot.leaf_count)
                .map_err(|_| "workspace generation leaf count overflow".to_owned())?,
            owner_count: u64::try_from(owners.len())
                .map_err(|_| "workspace generation owner count overflow".to_owned())?,
        };
    agent_semantic_content_identity::workspace_generation_evidence::ValidatedWorkspaceGenerationV1::new(
        workspace_generation.clone(),
    )
    .map_err(|error| format!("workspace overlay generation evidence is incomplete: {error}"))?;
    let generation_digest = super::registry::generation_digest(
        &workspace_identity,
        &source_snapshot,
        &workspace_generation,
        &owners,
    )?;
    Ok((
        active_epoch,
        WorkspaceMemoryGeneration {
            workspace_identity,
            state: WorkspaceGenerationState::Ready,
            active_epoch: active_epoch + 1,
            generation_digest: generation_digest.clone(),
            root_depth: [1, 0],
            workspace_snapshot,
            source_snapshot,
            workspace_generation,
            memory_backend_digest: generation_digest,
            owners,
        },
    ))
}
