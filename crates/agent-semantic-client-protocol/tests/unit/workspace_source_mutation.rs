// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-only

use crate::workspace_source_mutation::ChangedSourceOwner;
use crate::workspace_source_mutation::RemovedSourceOwner;
use crate::workspace_source_mutation::SourceOwnerPath;
use crate::workspace_source_mutation::SourceSnapshotDigest;
use crate::workspace_source_mutation::WorkspaceIdentity;
use crate::workspace_source_mutation::WorkspaceMutationId;
use crate::workspace_source_mutation::WorkspaceSourceMutation;
use crate::workspace_source_mutation::WorkspaceSourceMutationError;

fn owner_path(value: &str) -> SourceOwnerPath {
    SourceOwnerPath::new(value).expect("owner path")
}

#[test]
fn accepts_ordered_disjoint_owner_sets() {
    let mutation = WorkspaceSourceMutation::new(
        WorkspaceMutationId::new("mutation:1").expect("mutation id"),
        WorkspaceIdentity::new("workspace:1").expect("workspace identity"),
        None,
        vec![ChangedSourceOwner {
            owner_path: owner_path("src/lib.rs"),
            source_snapshot_digest: SourceSnapshotDigest::new("blake3-256:changed")
                .expect("source digest"),
        }],
        vec![RemovedSourceOwner {
            owner_path: owner_path("src/removed.rs"),
        }],
    )
    .expect("valid mutation");

    assert_eq!(
        mutation.schema_id,
        "agent.semantic-protocols.workspace-source-mutation"
    );
    assert_eq!(mutation.schema_version, "1");
}

#[test]
fn rejects_duplicate_or_unordered_changed_owners() {
    let result = WorkspaceSourceMutation::new(
        WorkspaceMutationId::new("mutation:1").expect("mutation id"),
        WorkspaceIdentity::new("workspace:1").expect("workspace identity"),
        None,
        vec![
            ChangedSourceOwner {
                owner_path: owner_path("src/z.rs"),
                source_snapshot_digest: SourceSnapshotDigest::new("digest:z")
                    .expect("source digest"),
            },
            ChangedSourceOwner {
                owner_path: owner_path("src/a.rs"),
                source_snapshot_digest: SourceSnapshotDigest::new("digest:a")
                    .expect("source digest"),
            },
        ],
        Vec::new(),
    );

    assert_eq!(
        result,
        Err(WorkspaceSourceMutationError::OwnersNotStrictlyOrdered {
            field: "changedOwners"
        })
    );
}

#[test]
fn rejects_changed_removed_overlap() {
    let result = WorkspaceSourceMutation::new(
        WorkspaceMutationId::new("mutation:1").expect("mutation id"),
        WorkspaceIdentity::new("workspace:1").expect("workspace identity"),
        None,
        vec![ChangedSourceOwner {
            owner_path: owner_path("src/lib.rs"),
            source_snapshot_digest: SourceSnapshotDigest::new("digest:changed")
                .expect("source digest"),
        }],
        vec![RemovedSourceOwner {
            owner_path: owner_path("src/lib.rs"),
        }],
    );

    assert_eq!(
        result,
        Err(WorkspaceSourceMutationError::ConflictingOwner {
            owner_path: "src/lib.rs".to_owned()
        })
    );
}

#[test]
fn rejects_empty_mutation_and_workspace_escape() {
    let empty = WorkspaceSourceMutation::new(
        WorkspaceMutationId::new("mutation:1").expect("mutation id"),
        WorkspaceIdentity::new("workspace:1").expect("workspace identity"),
        None,
        Vec::new(),
        Vec::new(),
    );
    assert_eq!(empty, Err(WorkspaceSourceMutationError::EmptyMutation));

    assert_eq!(
        SourceOwnerPath::new("src/../outside.rs"),
        Err(WorkspaceSourceMutationError::InvalidOwnerPath {
            owner_path: "src/../outside.rs".to_owned()
        })
    );
}
