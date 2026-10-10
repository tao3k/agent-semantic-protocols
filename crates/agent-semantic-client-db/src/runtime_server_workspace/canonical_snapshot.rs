// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Validates the complete source snapshot and its searchable owner subset.

use super::WorkspaceOwnerSnapshot;

pub(super) fn validate_canonical_snapshot(
    workspace_snapshot: &agent_semantic_content_identity::WorkspaceSnapshot,
    source_snapshot: &agent_semantic_content_identity::SourceSnapshotEvidence,
) -> Result<(), String> {
    if workspace_snapshot.root_digest() == source_snapshot.root_digest {
        return Ok(());
    }
    Err(format!(
        "workspace canonical materialization source snapshot drift: expected={} actual={}",
        workspace_snapshot.root_digest(),
        source_snapshot.root_digest
    ))
}

pub(super) fn validate_owner_snapshot_membership(
    workspace_snapshot: &agent_semantic_content_identity::WorkspaceSnapshot,
    owners: &[WorkspaceOwnerSnapshot],
) -> Result<(), String> {
    for owner in owners {
        let actual_digest =
            agent_semantic_content_identity::exact_selector_merkle::blake3_content_digest_v1(
                &owner.bytes,
            );
        let Some(expected_digest) = workspace_snapshot.file_digest(&owner.owner_path) else {
            return Err(format!(
                "workspace canonical materialization owner is absent from source snapshot: ownerPath={}",
                owner.owner_path
            ));
        };
        if expected_digest != actual_digest.as_str() {
            return Err(format!(
                "workspace canonical materialization owner digest drift: ownerPath={} expected={} actual={}",
                owner.owner_path,
                expected_digest,
                actual_digest.as_str()
            ));
        }
    }
    Ok(())
}

pub(super) fn validate_auxiliary_snapshot_membership(
    workspace_snapshot: &agent_semantic_content_identity::WorkspaceSnapshot,
    owners: &[super::WorkspaceAuxiliaryOwnerSnapshot],
) -> Result<(), String> {
    let mut paths = std::collections::BTreeSet::new();
    for owner in owners {
        if owner.owner_path.trim().is_empty() || !paths.insert(owner.owner_path.as_str()) {
            return Err("workspace auxiliary owner paths must be non-empty and unique".to_owned());
        }
        let actual =
            agent_semantic_content_identity::exact_selector_merkle::blake3_content_digest_v1(
                &owner.bytes,
            );
        if owner.content_digest.strip_prefix("blake3-256:") != Some(actual.as_str()) {
            return Err(format!(
                "workspace auxiliary owner content digest drift: ownerPath={}",
                owner.owner_path
            ));
        }
        if workspace_snapshot.file_digest(&owner.owner_path) != Some(actual.as_str()) {
            return Err(format!(
                "workspace auxiliary owner is absent from admitted snapshot: ownerPath={}",
                owner.owner_path
            ));
        }
    }
    Ok(())
}
