// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use super::{complete_active_source_blobs, partition_affected_owner_membership};
use std::collections::BTreeSet;

#[test]
fn replacement_membership_partitions_upserts_and_tombstones() {
    let affected = BTreeSet::from([
        "removed.rs".to_owned(),
        "replaced.rs".to_owned(),
        "unchanged-outside-provider.rs".to_owned(),
    ]);
    let present = BTreeSet::from([
        "new.rs".to_owned(),
        "replaced.rs".to_owned(),
        "unchanged-outside-provider.rs".to_owned(),
    ]);

    let (upserts, tombstones) = partition_affected_owner_membership(&affected, &present);

    assert_eq!(
        upserts,
        BTreeSet::from([
            "replaced.rs".to_owned(),
            "unchanged-outside-provider.rs".to_owned(),
        ])
    );
    assert_eq!(tombstones, BTreeSet::from(["removed.rs".to_owned()]));
    assert!(upserts.is_disjoint(&tombstones));
    assert!(!upserts.contains("new.rs"));
}

#[test]
fn canonical_auxiliary_leaves_survive_an_owner_local_successor() {
    let primary = b"fn primary() {}\n";
    let auxiliary = b"package: fixture\n";
    let primary_digest = format!("blake3-256:{}", blake3::hash(primary).to_hex());
    let auxiliary_digest = format!("blake3-256:{}", blake3::hash(auxiliary).to_hex());
    let snapshot = agent_semantic_content_identity::WorkspaceSnapshot::from_file_bytes([
        ("src/lib.rs", primary.as_slice()),
        ("gerbil.pkg", auxiliary.as_slice()),
    ]);
    let active = crate::ClientDbActiveGenerationSourceBlobs {
        generation_id: "generation-1".to_owned(),
        owners: vec![crate::ClientDbActiveGenerationSourceBlob {
            owner_path: "src/lib.rs".to_owned(),
            content_digest: primary_digest.clone(),
            source_bytes: std::sync::Arc::from(primary.as_slice()),
        }],
    };
    let completed = complete_active_source_blobs(
        &active,
        &snapshot,
        &[crate::runtime_server_workspace::WorkspaceOwnerSnapshot {
            owner_path: "src/lib.rs".to_owned(),
            authority: None,
            content_digest: primary_digest,
            native_syntax_diagnostic: None,
            bytes: primary.to_vec(),
            selectors: Vec::new(),
        }],
        &[
            crate::runtime_server_workspace::WorkspaceAuxiliaryOwnerSnapshot {
                owner_path: "gerbil.pkg".to_owned(),
                content_digest: auxiliary_digest,
                bytes: auxiliary.to_vec(),
            },
        ],
    )
    .expect("complete active source proof");

    assert_eq!(completed.owners.len(), 2);
    assert!(
        completed
            .owners
            .iter()
            .any(|owner| owner.owner_path == "gerbil.pkg")
    );
}
