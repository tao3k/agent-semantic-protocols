// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-only

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
