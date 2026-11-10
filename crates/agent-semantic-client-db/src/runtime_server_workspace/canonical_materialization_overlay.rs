// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Canonical owner-proof overlay for one immutable generation successor.

use std::collections::{BTreeMap, BTreeSet};

use super::WorkspaceCanonicalMaterialization;

pub(crate) struct WorkspaceCanonicalMaterializationOverlay<'a> {
    pub(crate) delta: &'a WorkspaceCanonicalMaterialization,
    pub(crate) workspace_snapshot: agent_semantic_content_identity::WorkspaceSnapshot,
    pub(crate) source_snapshot: agent_semantic_content_identity::SourceSnapshotEvidence,
    pub(crate) import: &'a crate::ClientDbSourceIndexImport,
    pub(crate) upsert_owner_paths: &'a BTreeSet<String>,
    pub(crate) removed_owner_paths: &'a BTreeSet<String>,
    pub(crate) project_resolutions: Vec<agent_semantic_content_identity::AdmittedProjectResolution>,
}

impl WorkspaceCanonicalMaterialization {
    pub(crate) fn from_source_index_overlay(
        mut active: Self,
        overlay: WorkspaceCanonicalMaterializationOverlay<'_>,
    ) -> Result<Self, String> {
        validate_delta_membership(
            overlay.delta,
            overlay.upsert_owner_paths,
            overlay.removed_owner_paths,
        )?;
        active.owners.retain(|owner| {
            !overlay.upsert_owner_paths.contains(&owner.owner_path)
                && !overlay.removed_owner_paths.contains(&owner.owner_path)
        });
        active.owners.extend(overlay.delta.owners.iter().cloned());
        active
            .owners
            .sort_by(|left, right| left.owner_path.cmp(&right.owner_path));

        let auxiliary_owners = overlay_auxiliary_owners(
            active.auxiliary_owners,
            &overlay.delta.auxiliary_owners,
            overlay.removed_owner_paths,
        );
        Self::new_with_workspace_snapshot(
            active.workspace_identity,
            overlay.workspace_snapshot,
            overlay.source_snapshot,
            overlay.import,
            active.root_depth,
            active.owners,
            auxiliary_owners,
            overlay.project_resolutions,
        )
    }
}

fn validate_delta_membership(
    delta: &WorkspaceCanonicalMaterialization,
    upsert_owner_paths: &BTreeSet<String>,
    removed_owner_paths: &BTreeSet<String>,
) -> Result<(), String> {
    if !upsert_owner_paths.is_disjoint(removed_owner_paths) {
        return Err("canonical materialization owner cannot be upserted and removed".to_owned());
    }
    if delta
        .owners
        .iter()
        .any(|owner| !upsert_owner_paths.contains(&owner.owner_path))
    {
        return Err("canonical materialization delta escaped changed owner membership".to_owned());
    }
    Ok(())
}

fn overlay_auxiliary_owners(
    active: Vec<super::WorkspaceAuxiliaryOwnerSnapshot>,
    delta: &[super::WorkspaceAuxiliaryOwnerSnapshot],
    removed_owner_paths: &BTreeSet<String>,
) -> Vec<super::WorkspaceAuxiliaryOwnerSnapshot> {
    let mut owners = active
        .into_iter()
        .filter(|owner| !removed_owner_paths.contains(&owner.owner_path))
        .map(|owner| (owner.owner_path.clone(), owner))
        .collect::<BTreeMap<_, _>>();
    owners.extend(
        delta
            .iter()
            .cloned()
            .map(|owner| (owner.owner_path.clone(), owner)),
    );
    owners.into_values().collect()
}
